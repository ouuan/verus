use crate::ast::{Command, CommandX, Declaration, Ident, Query, ValidityResult};
use crate::print_parse::Logger;
use std::collections::HashMap;
use z3::ast::Dynamic;
use z3::Sort;

pub struct Context<'ctx> {
    pub(crate) context: &'ctx z3::Context,
    pub(crate) solver: &'ctx z3::Solver<'ctx>,
    pub(crate) typs: HashMap<Ident, Sort<'ctx>>,
    pub(crate) vars: HashMap<Ident, Dynamic<'ctx>>,
    pub(crate) rlimit: u32,
    pub(crate) air_initial_log: Logger,
    pub(crate) air_final_log: Logger,
    pub(crate) smt_log: Logger,
}

impl<'ctx> Context<'ctx> {
    pub fn new(context: &'ctx z3::Context, solver: &'ctx z3::Solver<'ctx>) -> Context<'ctx> {
        Context {
            context,
            solver,
            typs: HashMap::new(),
            vars: HashMap::new(),
            rlimit: 0,
            air_initial_log: Logger::new(None),
            air_final_log: Logger::new(None),
            smt_log: Logger::new(None),
        }
    }

    pub fn set_air_initial_log(&mut self, writer: Box<dyn std::io::Write>) {
        self.air_initial_log = Logger::new(Some(writer));
    }

    pub fn set_air_final_log(&mut self, writer: Box<dyn std::io::Write>) {
        self.air_final_log = Logger::new(Some(writer));
    }

    pub fn set_smt_log(&mut self, writer: Box<dyn std::io::Write>) {
        self.smt_log = Logger::new(Some(writer));
    }

    pub fn set_rlimit(&mut self, rlimit: u32) {
        self.rlimit = rlimit;
        self.air_initial_log.log_set_option("rlimit", &rlimit.to_string());
        self.air_final_log.log_set_option("rlimit", &rlimit.to_string());
    }

    // emit blank line into log files
    pub fn blank_line(&mut self) {
        self.air_initial_log.blank_line();
        self.air_final_log.blank_line();
        self.smt_log.blank_line();
    }

    // Single-line comment, emitted with ";;" into log files
    pub fn comment(&mut self, s: &str) {
        self.air_initial_log.comment(s);
        self.air_final_log.comment(s);
        self.smt_log.comment(s);
    }

    fn log_set_z3_param(&mut self, option: &str, value: &str) {
        self.air_initial_log.log_set_option(option, value);
        self.air_final_log.log_set_option(option, value);
        self.smt_log.log_set_option(option, value);
    }

    pub(crate) fn set_z3_param_bool(&mut self, option: &str, value: bool, write_to_logs: bool) {
        if option == "air_recommended_options" && value {
            self.set_z3_param_bool("auto_config", false, true);
            self.set_z3_param_bool("smt.mbqi", false, true);
            self.set_z3_param_u32("smt.case_split", 3, true);
            self.set_z3_param_f64("smt.qi.eager_threshold", 100.0, true);
            self.set_z3_param_bool("smt.delay_units", true, true);
            self.set_z3_param_u32("smt.arith.solver", 2, true);
            self.set_z3_param_bool("smt.arith.nl", false, true);
        } else {
            let mut z3_params = z3::Params::new(&self.context);
            z3_params.set_bool(option, value);
            if write_to_logs {
                self.log_set_z3_param(option, &value.to_string());
            }
            self.solver.set_params(&z3_params);
        }
    }

    pub(crate) fn set_z3_param_u32(&mut self, option: &str, value: u32, write_to_logs: bool) {
        if option == "rlimit" && write_to_logs {
            self.set_rlimit(value);
        } else {
            let mut z3_params = z3::Params::new(&self.context);
            z3_params.set_u32(option, value);
            if write_to_logs {
                self.log_set_z3_param(option, &value.to_string());
            }
            self.solver.set_params(&z3_params);
        }
    }

    pub(crate) fn set_z3_param_f64(&mut self, option: &str, value: f64, write_to_logs: bool) {
        let mut z3_params = z3::Params::new(&self.context);
        z3_params.set_f64(option, value);
        if write_to_logs {
            let mut s = value.to_string();
            if !s.contains(".") {
                s += ".0";
            }
            self.log_set_z3_param(option, &s);
        }
        self.solver.set_params(&z3_params);
    }

    pub fn set_z3_param(&mut self, option: &str, value: &str) {
        if value == "true" {
            self.set_z3_param_bool(option, true, true);
        } else if value == "false" {
            self.set_z3_param_bool(option, true, true);
        } else if value.contains(".") {
            let v = value.parse::<f64>().expect(&format!("could not parse option value {}", value));
            self.set_z3_param_f64(option, v, true);
        } else {
            let v = value.parse::<u32>().expect(&format!("could not parse option value {}", value));
            self.set_z3_param_u32(option, v, true);
        }
    }

    pub fn push(&mut self) {
        self.air_initial_log.log_push();
        self.air_final_log.log_push();
        self.smt_log.log_push();
        self.solver.push();
    }

    pub fn pop(&mut self) {
        self.air_initial_log.log_pop();
        self.air_final_log.log_pop();
        self.smt_log.log_pop();
        self.solver.pop(1);
    }

    pub fn global(&mut self, decl: &Declaration) {
        self.air_initial_log.log_decl(decl);
        self.air_final_log.log_decl(decl);
        crate::smt_verify::smt_add_decl(self, &decl, true);
    }

    pub fn check_valid(&mut self, query: &Query) -> ValidityResult {
        self.air_initial_log.log_query(query);
        let query = crate::var_to_const::lower_query(&query);
        let query = crate::block_to_assert::lower_query(&query);
        self.air_final_log.log_query(&query);

        let validity = crate::smt_verify::smt_check_query(self, &query);

        validity
    }

    pub fn command(&mut self, command: &Command) -> ValidityResult {
        match &**command {
            CommandX::Push => {
                self.push();
                ValidityResult::Valid
            }
            CommandX::Pop => {
                self.push();
                ValidityResult::Valid
            }
            CommandX::SetOption(option, value) => {
                self.set_z3_param(option, value);
                ValidityResult::Valid
            }
            CommandX::Global(decl) => {
                self.global(&decl);
                ValidityResult::Valid
            }
            CommandX::CheckValid(query) => self.check_valid(&query),
        }
    }
}