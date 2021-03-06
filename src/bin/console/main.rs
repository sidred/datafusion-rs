// Copyright 2018 Grove Enterprises LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

extern crate clap;
extern crate datafusion;
extern crate liner;

use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::rc::Rc;
use std::str;
use std::time::Instant;

use clap::{App, Arg};
use datafusion::exec::*;
use datafusion::sqlast::ASTNode::SQLCreateTable;
use datafusion::sqlparser::*;
use datafusion::functions::geospatial::*;
use datafusion::functions::math::*;

mod linereader;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

fn main() {
    println!("DataFusion Console");

    let cmdline = App::new("DataFusion Console")
            .version(VERSION)
//            .arg(
//                Arg::with_name("ETCD")
//                    .help("etcd endpoints")
//                    .short("e")
//                    .long("etcd")
//                    .value_name("URL")
//                    .required(true)
//                    .takes_value(true),
//            )
            .arg(
                Arg::with_name("SCRIPT")
                    .help("SQL script to run")
                    .short("s")
                    .long("script")
                    .required(false)
                    .takes_value(true),
            )
            .get_matches();

    //parse args
    //let etcd_endpoints = cmdline.value_of("ETCD").unwrap();
    let mut console = Console::new(/*etcd_endpoints.to_string()*/);

    match cmdline.value_of("SCRIPT") {
        Some(filename) => match File::open(filename) {
            Ok(f) => {
                let mut reader = BufReader::new(&f);
                for line in reader.lines() {
                    match line {
                        Ok(cmd) => console.execute(&cmd),
                        Err(e) => println!("Error: {}", e),
                    }
                }
            }
            Err(e) => println!("Could not open file {}: {}", filename, e),
        },
        None => {
            let mut reader = linereader::LineReader::new();
            loop {
                let result = reader.read_lines();
                match result {
                    Some(line) => match line {
                        linereader::LineResult::Break => break,
                        linereader::LineResult::Input(command) => console.execute(&command),
                    },
                    None => (),
                }
            }
        }
    }
}

/// Interactive SQL console
struct Console {
    ctx: ExecutionContext,
}

impl Console {

    /// Create a new instance of the console
    fn new() -> Self {
        let mut ctx = ExecutionContext::local();
        ctx.register_scalar_function(Rc::new(STPointFunc{}));
        ctx.register_scalar_function(Rc::new(STAsText{}));
        ctx.register_scalar_function(Rc::new(SqrtFunction{}));
        Console { ctx }
    }

    /// Execute a SQL statement or console command
    fn execute(&mut self, sql: &str) {
        println!("Executing query ...");

        let timer = Instant::now();

        // parse the SQL
        match Parser::parse_sql(String::from(sql)) {
            Ok(ast) => match ast {
                SQLCreateTable { .. } => {
                    self.ctx.sql(&sql).unwrap();
                    //println!("Registered schema with execution context");
                    ()
                }
                _ => match self.ctx.create_logical_plan(sql) {
                    Ok(logical_plan) => {
                        let physical_plan = PhysicalPlan::Interactive {
                            plan: logical_plan.clone(),
                        };

                        let result = self.ctx.execute(&physical_plan);

                        match result {
                            Ok(result) => {
                                let elapsed = timer.elapsed();
                                let elapsed_seconds = elapsed.as_secs() as f64
                                    + elapsed.subsec_nanos() as f64 / 1000000000.0;

                                match result {
                                    ExecutionResult::Unit => {
                                        println!("Query executed in {} seconds", elapsed_seconds);
                                    }
                                    ExecutionResult::Count(n) => {
                                        println!(
                                            "Query executed in {} seconds and updated {} rows",
                                            elapsed_seconds, n
                                        );
                                    }
                                }
                            }
                            Err(e) => println!("Error: {:?}", e),
                        }
                    }
                    Err(e) => println!("Error: {:?}", e),
                },
            },
            Err(e) => println!("Error: {:?}", e),
        }
    }
}
