#![recursion_limit = "1024"]

#[macro_use]
extern crate error_chain;
extern crate nix;

use std::io::Read;
use std::io::Write;
use nix::sys::termios;

use nix::libc::STDIN_FILENO;

mod errors {
    // Create the Error, ErrorKind, ResultExt, and Result types
    error_chain!{
        foreign_links {
            Nix(::nix::Error);
        }
    }
}
use errors::*;

/** constants **/
const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const ESCAPE: char = '\x1b';

/** terminal**/
struct EditorConfig {
    orig: termios::Termios,
    rows: u16,
    cols: u16,
    cx: u16,
    cy: u16,
}

fn ctrl(c: char) -> u8 {
    (c as u8) & 0x1f
}

fn enable_raw_mode() -> Result<()> {
    use termios::*;

    let mut raw = tcgetattr(STDIN_FILENO)?;
    raw.c_iflag.remove(BRKINT | ICRNL | INPCK | IXON);
    raw.c_oflag.remove(OPOST);
    raw.c_cflag.insert(CS8);
    raw.c_lflag.remove(ECHO | ICANON | IEXTEN | ISIG);
    tcsetattr(STDIN_FILENO, TCSAFLUSH, &raw)?;

    Ok(())
}

fn read_window_size() -> Result<(u16, u16)> {
    use nix::libc::*;

    unsafe {
        let mut wc = winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        ioctl(STDOUT_FILENO, TIOCGWINSZ, &mut wc);
        if wc.ws_col > 0 && wc.ws_row > 0 {
            return Ok((wc.ws_row as u16, wc.ws_col as u16));
        }
    }
    Err("Unable to read terminal size".into())
}

fn term_config() -> Result<EditorConfig> {
    let orig = termios::tcgetattr(STDIN_FILENO)?;
    let (rows, cols) = read_window_size()?;
    Ok(EditorConfig {
        orig: orig,
        rows: rows,
        cols: cols,
        cx: 0,
        cy: 0,
    })
}

/** input **/
#[derive(Debug, Clone, Copy)]
enum Input {
    Control(u8),
    Char(char),
    ArrowUp,
    ArrowDown,
    ArrowRight,
    ArrowLeft
}

fn process_key(i: Input, conf: &mut EditorConfig) -> bool {
    use Input::*;
    match i {
        Control(b) if b == ctrl('q') => return false,
        Control(b) => print!("{}\r\n", b),
        Char(c) => print!("{} ({})\r\n", c as u8, c), 
        ArrowUp => {
            if conf.cy > 0 {
                conf.cy -= 1
            }
        }
        ArrowDown => {
            if conf.cy < conf.rows - 1 {
                conf.cy += 1
            }
        }
        ArrowLeft => {
            if conf.cx > 0 {
                conf.cx -= 1
            }
        }
        ArrowRight => {
            if conf.cx < conf.cols - 1 {
                conf.cx += 1
            }
        }
    }
    true
}

/** output **/
fn draw_rows(conf: &EditorConfig) {
    let mut buf = String::new();
    buf += "\x1b[?25l";             // Hide cursor
    buf += "\x1b[H";                // Move cursor to top-right
    for y in 0..conf.rows {
        if y == conf.rows / 3 {
            let welcome = format!("Welcome to Fanto editor version {}", VERSION);
            let len = std::cmp::min(conf.cols as usize, welcome.len());
            let padding = (conf.cols as usize - len) / 2;
            if padding > 0 {
                buf += "~";
                buf += &std::iter::repeat(" ").take(padding - 1).collect::<String>();
            }
            buf += welcome.split_at(len - 1).0;
        } else {
            buf += "~";
        }
        buf += "\x1b[K";
        if y < conf.rows - 1 {
            buf += "\r\n";
        }
    }
    buf += &format!("\x1b[{};{}H", conf.cy + 1, conf.cx + 1);
    buf += "\x1b[?25h";             // Show cursor
    print!("{}", buf);
    let _ = std::io::stdout().flush();
}

fn refresh_screen() {
    print!("\x1b[2J\x1b[H");
    let _ = std::io::stdout().flush();
}

fn read_key<I>(bytes: &mut I) -> Option<Input>
    where I: Iterator<Item = std::io::Result<u8>>
{
    if let Some(Ok(b)) = bytes.next() {
        return match b as char {
            ESCAPE => {
                if let (Some(Ok(b1)), Some(Ok(b2))) = (bytes.next(), bytes.next()) {
                    match (b1 as char, b2 as char) {
                        ('[', 'A') => Some(Input::ArrowUp),
                        ('[', 'B') => Some(Input::ArrowDown),
                        ('[', 'C') => Some(Input::ArrowRight),
                        ('[', 'D') => Some(Input::ArrowLeft),
                        (c, _) => Some(Input::Char(c))
                    }
                } else {
                    Some(Input::Control(ESCAPE as u8))
                }
            }
            c if c.is_control() => Some(Input::Control(b)),
            c => Some(Input::Char(c)),
        };
    }
    return None;
}

/** main function **/
fn run() -> Result<()> {
    let mut config = term_config().chain_err(|| "Unable to initialize terminal config")?;

    let stdin = std::io::stdin();
    let mut bytes = stdin.lock().bytes();
    enable_raw_mode()?;
    draw_rows(&config);

    while let Some(i) = read_key(&mut bytes) {
        if !process_key(i, &mut config) {
            break;
        }
        // draw_rows(&config);
    }
    println!();

    termios::tcsetattr(STDIN_FILENO, termios::TCSAFLUSH, &config.orig)?;
    Ok(())
}

fn main() {
    let res = run();
    refresh_screen();
    if let Err(ref e) = res {
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
