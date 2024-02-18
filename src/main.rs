use clap::Parser;
use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::str;
use walkdir::WalkDir;

fn parse_dir(s: &str) -> Result<String, String> {
    if s.is_empty() {
        return Ok(env::var("RANCH_DIR").unwrap_or_else(|_| {
            env::var("STOW_DIR").unwrap_or_else(|_| {
                String::from(
                    env::current_dir()
                        .expect("Could not open the current directory.")
                        .into_os_string()
                        .into_string()
                        .expect("Could not decode 'dir'; invalid unicode encountered."),
                )
            })
        }));
    }
    return Ok(String::from(s));
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(
    short = 'n',
    long = "no",
    alias = "dry-run",
    default_value_t = false,
    help = "Do not perform any operations that modify the filesystem; merely show what would happen"
    )]
    dry_run: bool,

    #[arg(short = 'd', long, alias = "C", default_value = "", value_parser = parse_dir)]
    dir: String,
    // #[arg(short = 'C', long, default_value_t = String::from(TEST_ROOT))]
    // chdir: String,
    // #[arg(short = 'x', long, default_values_t = [String::from(".git")])]
    // exclude: Vec<String>,
}

fn find_files(root: &Path, excludes: &[OsString]) -> Vec<PathBuf> {
    WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        // .filter_entry() //https://github.com/BurntSushi/walkdir/blob/master/README.md#example-skip-hidden-files-and-directories-efficiently-on-unix
        .filter_map(|r| r.ok())
        .map(|e| e.path().to_path_buf())
        .filter(|e| {
            !excludes
                .iter()
                .any(|x| x == e.file_name().unwrap_or(OsStr::new("")))
        })
        .collect()
}

fn main() {
    let args = Args::parse();

    println!("{}", args.dir);

    // let exclude: Vec<OsString> = args.exclude.iter().map(|s| s.into()).collect();
    //
    // for p in find_files(Path::new(&args.chdir), &exclude) {
    //     println!("{:?}", p);
    // }
}
