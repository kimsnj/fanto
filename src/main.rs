#![recursion_limit = "1024"]

#[macro_use]
extern crate error_chain;
extern crate nix;

use std::io::Read;
use nix::sys::termios;

mod errors {
    // Create the Error, ErrorKind, ResultExt, and Result types
    error_chain!{
        foreign_links {
            Nix(::nix::Error);
        }
    }
}
use errors::*;

fn main() {
    if let Err(ref e) = run() {
        use std::io::Write;
        let stderr = &mut ::std::io::stderr();
        let errmsg = "Error writing to stderr";

        writeln!(stderr, "error: {}", e).expect(errmsg);

        for e in e.iter().skip(1) {
            writeln!(stderr, "caused by: {}", e).expect(errmsg);
        }

        // The backtrace is not always generated. Try to run this example
        // with `RUST_BACKTRACE=1`.
        if let Some(backtrace) = e.backtrace() {
            writeln!(stderr, "backtrace: {:?}", backtrace).expect(errmsg);
        }

        ::std::process::exit(1);
    }
}

fn enable_raw_mode() -> Result<()> {
    use termios::*;

    let mut raw = tcgetattr(0)?;
    raw.c_iflag.remove(BRKINT | ICRNL | INPCK | IXON);
    raw.c_oflag.remove(OPOST);
    raw.c_cflag.insert(CS8);
    raw.c_lflag.remove(ECHO | ICANON | IEXTEN | ISIG);
    tcsetattr(0, TCSAFLUSH, &raw)?;
    Ok(())
}

fn run() -> Result<()> {
    let origin = termios::tcgetattr(0)?;
    let stdin = std::io::stdin();
    let mut bytes = stdin.lock().bytes();
    enable_raw_mode()?;

    while let Some(Ok(b)) = bytes.next() {
        match b as char {
            'q' => break,
            c if c.is_control() => print!("{}\r\n", b),
            c => print!("{} ({})\r\n", b, c),
        }
    }
    println!();

    termios::tcsetattr(0, termios::TCSAFLUSH, &origin)?;
    Ok(())
}
