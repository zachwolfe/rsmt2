#![ doc = "
Implements all the printing and parsing methods for the SMT lib 2 standard.
"]


use std::process::{ Command, Child, Stdio } ;
use std::io ;
use std::io::{ Write, Read } ;
use std::process ;
use std::ffi::OsStr ;
use std::convert::AsRef ;


use common::* ;
use conf::SolverConf ;
use parse::* ;

macro_rules! try_writer {
  ($e:expr) => (
    match $e {
      Some(writer) => writer,
      None => return Err(
        io::Error::new(
          io::ErrorKind::Other, "cannot access reader of child process"
        )
      ),
    }
  )
}

/** Contains the actual solver process. */
pub struct Solver<Parser> {
  /** The command used to run the solver. */
  cmd: Command,
  /** The actual solver child process. */
  kid: Kid,
  /** The solver specific information. */
  conf: SolverConf,
  /** The parser. */
  parser: Parser,
}

impl<
  /* Parsing-related type parameters. */
  // Ident, Value, Expr, Proof,
  Parser
> Solver<Parser> {

  /** Creates a new solver. */
  pub fn mk(
    cmd: Command, conf: SolverConf, parser: Parser
  ) -> io::Result<Self> {
    let mut cmd = cmd ;
    /* Adding configuration options to the command. */
    cmd.args(conf.options()) ;
    /* Setting up pipes for child process. */
    cmd.stdin(Stdio::piped()) ;
    cmd.stdout(Stdio::piped()) ;
    cmd.stderr(Stdio::piped()) ;
    /* Spawning child process. */
    match cmd.spawn() {
      Ok(kid) => Ok(
        // Successful, creating solver.
        Solver {
          cmd: cmd, kid: Kid::mk(kid), conf: conf, parser: parser
        }
      ),
      Err(e) => Err(e),
    }
  }

  /** Kills the underlying solver. */
  pub fn kill(self) -> (Command, SolverConf) {
    let (cmd,conf,kid) = (self.cmd, self.conf, self.kid) ;
    // Do something about this ignored result.
    kid.kill() ;
    (cmd, conf)
  }

  /** Returns a pointer to the writer on the stdin of the process. */
  #[inline(always)]
  fn writer(& mut self) -> Option<& mut process::ChildStdin> {
    self.kid.stdin()
  }

  /** Returns a pointer to the reader on the stdout of the process. */
  #[inline(always)]
  fn out_reader(& mut self) -> Option<& mut process::ChildStdout> {
    self.kid.stdout()
  }

  /** Returns a pointer to the reader on the stderr of the process. */
  #[inline(always)]
  fn err_reader(& mut self) -> Option<& mut process::ChildStderr> {
    self.kid.stderr()
  }

  /** Gets the standard error output of the process as a string. */
  #[inline]
  pub fn out_as_string(& mut self) -> IoRes<String> {
    let mut s = String::new() ;
    match self.out_reader() {
      Some(stdout) => match stdout.read_to_string(& mut s) {
        Ok(_) => Ok(s),
        Err(e) => Err(e),
      },
      None => Err(
        io::Error::new(
          io::ErrorKind::Other, "cannot access stdout of child process"
        )
      ),
    }
  }

  /** Gets the standard error output of the process as a string. */
  #[inline]
  pub fn err_as_string(& mut self) -> io::Result<String> {
    let mut s = String::new() ;
    match self.err_reader() {
      Some(stdout) => match stdout.read_to_string(& mut s) {
        Ok(_) => Ok(s),
        Err(e) => Err(e),
      },
      None => Err(
        io::Error::new(
          io::ErrorKind::Other, "cannot access stderr of child process"
        )
      ),
    }
  }


  // Comment things.

  /** Prints some lines as SMT lib 2 comments. */
  pub fn comments(
    & mut self, lines: ::std::str::Lines
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    for line in lines { try!( write!(writer, ";; {}\n", line) ) } ;
    write!(writer, "\n")
  }
  /** Prints some text as comments. Input is sanitized in case it contains
  newlines. */
  pub fn comment(& mut self, txt: & str) -> IoResUnit {
    self.comments(txt.lines())
  }


  // |===| (Re)starting and terminating.

  /** Resets the underlying solver. Restarts the kid process if no reset
  command is supported. */
  pub fn reset(
    & mut self
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    write!(writer, "(reset)\n\n")
  }
  /** Sets the logic. */
  pub fn set_logic(
    & mut self, logic: & Logic
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    try!( write!(writer, "(set-logic ") ) ;
    try!( logic.to_smt2(writer, ()) ) ;
    write!(writer, ")\n\n")
  }
  /** Set option command. */
  pub fn set_option<Value: ::std::fmt::Display>(
    & mut self, option: & str, value: Value
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    write!(writer, "(set-option {} {})\n\n", option, value)
  }
  /** Activates interactive mode. */
  pub fn interactive_mode(& mut self) -> IoResUnit {
    self.set_option(":interactive-mode", true)
  }
  /** Activates print success. */
  pub fn print_success(& mut self) -> IoResUnit {
    self.set_option(":print-success", true)
  }
  /** Activates unsat core production. */
  pub fn print_unsat_core(& mut self) -> IoResUnit {
    self.set_option(":produce-unsat-cores", true)
  }
  /** Shuts the solver down. */
  pub fn exit(
    & mut self
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    write!(writer, "(exit)\n")
  }


  // |===| Modifying the assertion stack.

  /** Pushes `n` layers on the assertion stack. */
  pub fn push(
    & mut self, n: & u8
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    write!(writer, "(push {})\n\n", n)
  }
  /** Pops `n` layers off the assertion stack. */
  pub fn pop(
    & mut self, n: & u8
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    write!(writer, "(pop {})\n\n", n)
  }
  /** Resets the assertions in the solver. */
  pub fn reset_assertions(
    & mut self
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    write!(writer, "(reset-assertions)\n\n")
  }


  // |===| Introducing new symbols.

  /** Declares a new sort. */
  pub fn declare_sort<Sort: PrintSmt2<()>>(
    & mut self, sort: Sort, arity: & u8
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    try!( write!(writer, "(declare-sort ") ) ;
    try!( sort.to_smt2(writer, ()) ) ;
    write!(writer, " {})\n\n", arity)
  }
  /** Defines a new sort. */
  pub fn define_sort<Sort: PrintSmt2<()>, Expr: PrintSmt2<()>>(
    & mut self, sort: Sort, args: & [ Expr ], body: Expr
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    try!( write!(writer, "( define-sort ") ) ;
    try!( sort.to_smt2(writer, ()) ) ;
    try!( write!(writer, "\n   ( ") ) ;
    for arg in args {
      try!( arg.to_smt2(writer, ()) ) ;
      try!( write!(writer, " ") )
    } ;
    try!( write!(writer, ")\n   ") ) ;
    try!( body.to_smt2(writer, ()) ) ;
    write!(writer, "\n)\n\n")
  }
  /** Declares a new function symbol. */
  pub fn declare_fun<
    IdentPrintInfo,
    Sort: PrintSmt2<()>, Ident: PrintSmt2<IdentPrintInfo>
  > (
    & mut self, symbol: & Ident, info: IdentPrintInfo,
    args: & [ Sort ], out: Sort
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    try!( write!(writer, "(declare-fun ") ) ;
    try!( symbol.to_smt2(writer, info) ) ;
    try!( write!(writer, " ( ") ) ;
    for arg in args {
      try!( arg.to_smt2(writer, ()) ) ;
      try!( write!(writer, " ") )
    } ;
    try!( write!(writer, ") ") ) ;
    try!( out.to_smt2(writer, ()) ) ;
    write!(writer, ")\n\n")
  }
  /** Declares a new function symbol, no info version. */
  #[inline(always)]
  pub fn ninfo_declare_fun<
    Sort: PrintSmt2<()>, Ident: PrintSmt2<()>
  > ( & mut self, symbol: & Ident, args: & [ Sort ], out: Sort ) -> IoResUnit {
    self.declare_fun(symbol, (), args, out)
  }
  /** Declares a new constant. */
  pub fn declare_const<Sort: PrintSmt2<()>, Ident: PrintSmt2<()>>(
    & mut self, symbol: Ident, out_sort: Sort
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    try!( write!(writer, "(declare-const ") ) ;
    try!( symbol.to_smt2(writer, ()) ) ;
    try!( write!(writer, " ") ) ;
    try!( out_sort.to_smt2(writer, ()) ) ;
    write!(writer, ")\n\n")
  }
  /** Defines a new function symbol. */
  pub fn define_fun<
    Sort: PrintSmt2<()>, Ident: PrintSmt2<()>, Expr: PrintSmt2<()>
  >(
    & mut self,
    symbol: Ident, args: & [ (Ident, Sort) ], out: Sort, body: Expr
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    try!( write!(writer, "(define-fun ") ) ;
    try!( symbol.to_smt2(writer, ()) ) ;
    try!( write!(writer, " ( ") ) ;
    for arg in args {
      let (ref id, ref sort) = * arg ;
      try!( write!(writer, "(") ) ;
      try!( id.to_smt2(writer, ()) ) ;
      try!( write!(writer, " ") ) ;
      try!( sort.to_smt2(writer, ()) ) ;
      try!( write!(writer, ") ") )
    } ;
    try!( write!(writer, ") ") ) ;
    try!( out.to_smt2(writer, ()) ) ;
    try!( write!(writer, "\n   ") ) ;
    try!( body.to_smt2(writer, ()) ) ;
    write!(writer, "\n)\n\n")
  }
  /** Defines some new (possibily mutually) recursive functions. */
  pub fn define_funs_rec<
    Sort: PrintSmt2<()>, Ident: PrintSmt2<()>, Expr: PrintSmt2<()>
  >(
    & mut self, funs: & [ (Ident, & [ (Ident, Sort) ], Sort, Expr) ]
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    // Header.
    try!( write!(writer, "(define-funs-rec (\n") ) ;

    // Signatures.
    for fun in funs {
      let (ref id, ref args, ref out, _) = * fun ;
      try!( write!(writer, "   (") ) ;
      try!( id.to_smt2(writer, ()) ) ;
      try!( write!(writer, " ( ") ) ;
      for arg in * args {
        let (ref arg, ref sort) = * arg ;
        try!( write!(writer, "(") ) ;
        try!( arg.to_smt2(writer, ()) ) ;
        try!( write!(writer, " ") ) ;
        try!( sort.to_smt2(writer, ()) ) ;
        try!( write!(writer, ") ") ) ;
      } ;
      try!( write!(writer, ") ") ) ;
      try!( out.to_smt2(writer, ()) ) ;
      try!( write!(writer, ")\n") )
    } ;
    try!( write!(writer, " ) (") ) ;

    // Bodies
    for fun in funs {
      let (_, _, _, ref body) = * fun ;
      try!( write!(writer, "\n   ") ) ;
      try!( body.to_smt2(writer, ()) )
    } ;
    write!(writer, "\n )\n)\n\n")
  }
  /** Defines a new recursive function. */
  pub fn define_fun_rec<
    Sort: PrintSmt2<()>, Ident: PrintSmt2<()>, Expr: PrintSmt2<()>
  >(
    & mut self,  symbol: Ident, args: & [ (Ident, Sort) ], out: Sort, body: Expr
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    // Header.
    try!( write!(writer, "(define-fun-rec (\n") ) ;

    // Signature.
    try!( write!(writer, "   (") ) ;
    try!( symbol.to_smt2(writer, ()) ) ;
    try!( write!(writer, " ( ") ) ;
    for arg in args {
      let (ref arg, ref sort) = * arg ;
      try!( write!(writer, "(") ) ;
      try!( arg.to_smt2(writer, ()) ) ;
      try!( write!(writer, " ") ) ;
      try!( sort.to_smt2(writer, ()) ) ;
      try!( write!(writer, ") ") ) ;
    } ;
    try!( write!(writer, ") ") ) ;
    try!( out.to_smt2(writer, ()) ) ;
    try!( write!(writer, ")\n") ) ;
    try!( write!(writer, " ) (") ) ;

    // Body.
    try!( write!(writer, "\n   ") ) ;
    try!( body.to_smt2(writer, ()) ) ;
    write!(writer, "\n )\n)\n\n")
  }


  // |===| Asserting and inspecting formulas.

  /** Asserts an expression with some print information. */
  pub fn assert<PrintInfo, Expr: PrintSmt2<PrintInfo>>(
    & mut self, expr: & Expr, info: PrintInfo
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    try!( write!(writer, "(assert\n  ") ) ;
    try!( expr.to_smt2(writer, info) ) ;
    write!(writer, "\n)")
  }
  /** Asserts an expression without any print info. `ninfo` = no info. */
  pub fn ninfo_assert<Expr: PrintSmt2<()>>(
    & mut self, expr: & Expr
  ) -> IoResUnit {
    self.assert(expr, ())
  }
  /** Get assertions command. */
  pub fn get_assertions(
    & mut self
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    write!(writer, "(get-assertions)\n\n")
  }


  // |===| Checking for satisfiability.

  /** Check-sat command. */
  pub fn check_sat(& mut self) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    write!(writer, "(check-sat)\n\n")
  }
  /** Check-sat assuming command. */
  pub fn check_sat_assuming<PrintInfo: Copy, Ident: PrintSmt2<PrintInfo>>(
    & mut self, bool_vars: & [ Ident ], info: PrintInfo
  ) -> IoResUnit {
    match self.conf.check_sat_assuming() {
      & Ok(cmd) => {
        let mut writer = try_writer!( self.writer() ) ;
        try!( write!(writer, "({}\n ", cmd) );
        for v in bool_vars {
          try!( write!(writer, " ") ) ;
          try!( v.to_smt2(writer, info) )
        } ;
        write!(writer, "\n)\n\n")
      },
      _ => panic!("check-sat-assuming command is not supported")
    }
  }
  /** Check-sat assuming command, no info version. */
  pub fn ninfo_check_sat_assuming<Ident: PrintSmt2<()>>(
    & mut self, bool_vars: & [ Ident ]
  ) -> IoResUnit {
    self.check_sat_assuming(bool_vars, ())
  }


  // |===| Inspecting models.

  /** Get value command. */
  pub fn get_value<PrintInfo: Copy, Expr: PrintSmt2<PrintInfo>>(
    & mut self, exprs: & [ Expr ], info: PrintInfo
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    try!( write!(writer, "(get-value") ) ;
    for e in exprs {
      try!( write!(writer, "\n  ") ) ;
      try!( e.to_smt2(writer, info) )
    } ;
    write!(writer, "\n)\n\n")
  }
  /** Get value command, no info version. */
  pub fn ninfo_get_value<Expr: PrintSmt2<()>>(
    & mut self, exprs: & [ Expr]
  ) -> IoResUnit {
    self.get_value(exprs, ())
  }
  /** Get assignment command. */
  pub fn get_assignment(& mut self) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    write!(writer, "(get-assignment)\n\n")
  }
  /** Get model command. */
  pub fn get_model(& mut self) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    write!(writer, "(get-model)\n\n")
  }


  // |===| Inspecting proofs.

  /** Get unsat assumptions command. */
  pub fn get_unsat_assumptions(
    & mut self
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    write!(writer, "(get-unsat-assumptions)\n\n")
  }
  /** Get proof command. */
  pub fn get_proof(
    & mut self
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    write!(writer, "(get-proof)\n\n")
  }
  /** Get unsat core command. */
  pub fn get_unsat_core(
    & mut self
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    write!(writer, "(get-unsat-core)\n\n")
  }

  // |===| Inspecting settings.

  /** Get info command. */
  pub fn get_info(
    & mut self, flag: & str
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    write!(writer, "(get-info {})\n\n", flag)
  }
  /** Get option command. */
  pub fn get_option(
    & mut self, option: & str
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    write!(writer, "(get-option {})\n\n", option)
  }

  // |===| Script information.

  /** Set info command. */
  pub fn set_info(
    & mut self, attribute: & str
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    write!(writer, "(set-info {})\n\n", attribute)
  }
  /** Echo command. */
  pub fn echo(
    & mut self, text: & str
  ) -> IoResUnit {
    let mut writer = try_writer!( self.writer() ) ;
    write!(writer, "(echo \"{}\")\n\n", text)
  }


  // |===| Parsing simple stuff.

  pub fn parse_success(& mut self) -> SuccessRes {
    use nom::{ ReadProducer, Stepper } ;
    use nom::StepperState::* ;
    let producer = ReadProducer::new(& mut self.kid, 1000) ;
    let mut stepper = Stepper::new(producer) ;
    loop {
      match stepper.step( success ) {
        Value( Ok(()) ) => println!("parsed success"),
        Value( Err(e) ) => println!("error: {:?}", e),
        step => { println!("step = {:?}", step) ; break },
      }
    }
    Ok(())
  }

  pub fn parse_check_sat(& mut self) -> CheckSatRes {
    use nom::{ ReadProducer, Stepper } ;
    use nom::StepperState::* ;
    let producer = ReadProducer::new(& mut self.kid, 1000) ;
    let mut stepper = Stepper::new(producer) ;
    loop {
      match stepper.step( check_sat ) {
        Value( res ) => return res,
        Continue => { println!("continue") },
        step => panic!("{:?}", step),
      }
    }
    Ok(false)
  }
}



// impl<
//   Ident, Value, Expr, Proof, Parser: ParseSmt2<Ident, Value, Expr, Proof>
// > Solver<Parser> {


// }



// /** Can parse the result of SMT lib 2 queries. */
// impl<
//   Ident, Value, Expr, Proof
// > Smt2Parse<Ident, Value, Expr, Proof> for Solver {

//   fn parse_assertions(& mut self) -> IoRes<Option<Vec<Expr>>> {
//     Ok(None)
//   }

//   fn parse_value(& mut self) -> IoRes<Option<Vec<Value>>> {
//     Ok(None)
//   }
//   fn parse_assignment(& mut self) -> IoRes<Option<Vec<(Ident, Value)>>> {
//     Ok(None)
//   }
//   fn parse_model(& mut self) -> IoRes<Option<Vec<(Ident, Value)>>> {
//     Ok(None)
//   }

//   fn parse_unsat_assumptions(& mut self) -> IoRes<Option<Vec<Ident>>> {
//     Ok(None)
//   }
//   fn parse_proof(& mut self) -> IoRes<Option<Proof>> {
//     Ok(None)
//   }
//   fn parse_unsat_core(& mut self) -> IoRes<Option<Vec<Ident>>> {
//     Ok(None)
//   }

//   fn parse_info(& mut self) -> IoRes<Option<String>> {
//     Ok(None)
//   }
//   fn parse_option(& mut self) -> IoRes<Option<String>> {
//     Ok(None)
//   }

// }

// impl<
//   Ident: Printable,
//   Value,
//   Sort: Printable,
//   Expr: Printable,
//   Proof,
// > Smt2GetNow<Ident, Value, Sort, Expr, Proof> for Solver {}

// impl<
//   Ident: Printable,
//   Value,
//   Sort: Printable,
//   Expr: Printable,
//   Proof,
// > Smt2Solver<Ident, Value, Sort, Expr, Proof> for Solver {}


