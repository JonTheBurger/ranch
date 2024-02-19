use clap::Parser;
use std::env;
use std::io;
use std::path::PathBuf;
use std::process::exit;
use std::str;
use walkdir::WalkDir;

const LV_WARN: u8 = 1;
const LV_INFO: u8 = 2;
const LV_DEBUG: u8 = 3;

fn parse_dir(s: &str) -> Result<String, String> {
    if s.is_empty() {
        return Ok(env::var("RANCH_DIR").unwrap_or_else(|_| {
            String::from(
                env::current_dir()
                    .expect("FATAL: Could not open the current directory.")
                    .into_os_string()
                    .into_string()
                    .expect("FATAL: Could not decode 'dir'; invalid unicode encountered."),
            )
        }));
    }
    return Ok(String::from(s));
}

#[derive(Parser, Debug)]
#[command(
    author = "Jonathan Povirk",
    version,
    about = "Symlink farm inspired by GNU stow.",
    long_about = "Symlink farmer inspired by GNU stow.

Many applications store user-specific configuration files within the user's '$HOME' directory (or '%UserProfile%/%AppData%/%LocalAppData%' on Windows). \
Instead of copying these files between machines, ranch allows users to create softlinks for these files that point back to a centralized, version-controlled repository. \
Consider the following example:

  lrwxrwxrwx  1 alice alice    25 Aug 12  2022 .tmux.conf -> .dotfiles/home/.tmux.conf
  lrwxrwxrwx  1 alice alice    21 Aug 12  2022 .vimrc -> .dotfiles/home/.vimrc
  lrwxrwxrwx  1 alice alice    21 Aug 12  2022 .zshrc -> .dotfiles/home/.zshrc

All of these listed files point back to the .dotfiles repo, and updating is as simple as a 'git pull'. \
This program implements a subset of stow - notably, '--no-folding' is set as the default. \
In other words, ranch does not create symlinks of directories - only files. \
Intermediate directories will be created at the target location.
"
)]
struct Args {
    #[arg(
        short = 'n',
        long = "dry-run",
        alias = "no",
        default_value_t = false,
        help = "Do not perform any operations that modify the filesystem; merely show what would happen"
    )]
    dry_run: bool,

    #[arg(
    short = 'd',
    long,
    alias = "C",
    default_value = "",
    value_parser = parse_dir,
    help = "Change directory to 'DIR' to search for packages instead of using the current directory",
    )]
    dir: String,

    #[arg(
        short = 't',
        long,
        help = "Destination directory where symlinks are deployed; default implies 'DIR/..'"
    )]
    target: Option<String>,

    #[arg(
    short = 'v',
    long,
    action = clap::ArgAction::Count,
    help = "Standard error output verbosity (nothing by default); specify multiple times to print more",
    )]
    verbose: u8,

    #[arg(
        short = 'D',
        long,
        default_value = "",
        help = "Deletes the package from the target dir; only symlinks are deleted"
    )]
    delete: String,

    #[arg(help = "Name of a subdirectory of 'DIR' containing files to symlink")]
    package: String,
}

#[cfg(windows)]
fn soft_link(from: &PathBuf, to: &PathBuf) -> io::Result<()> {
    std::os::windows::fs::symlink_file(from, to)
}

#[cfg(unix)]
fn soft_link(from: &PathBuf, to: &PathBuf) -> io::Result<()> {
    std::os::unix::fs::symlink(from, to)
}

fn main() {
    let args = Args::parse();
    if args.verbose >= LV_DEBUG {
        eprintln!("{:?}", &args);
    }
    // --target's default is dependent the arg 'dir', so setup default value here.
    let target_path = match &args.target {
        Some(target) => PathBuf::from(target),
        _ => PathBuf::from(&args.dir)
            .parent()
            .expect("FATAL: Could not access default target path 'DIR/..'")
            .to_owned(),
    };

    // Check source path
    let prefix_path = PathBuf::from(&args.dir).join(&args.package);
    if args.verbose >= LV_INFO {
        eprintln!(
            "Linking... {} -> {}",
            &prefix_path.display(),
            &target_path.display()
        );
    }
    if !prefix_path.exists() {
        eprintln!(
            "FATAL: Package {} does not exist; exiting now",
            args.package
        );
        exit(1);
    }

    // Check destination path
    std::fs::create_dir_all(&target_path).expect("FATAL: Could not create target directory");

    // Make links
    for p in WalkDir::new(prefix_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|r| r.ok())
        .map(|e| e.path().to_path_buf())
    {
        if !(p.is_file() || p.is_symlink()) {
            continue;
        }

        let rel_path = p.strip_prefix(&args.dir);
        if !rel_path.is_ok() {
            if args.verbose >= LV_WARN {
                eprintln!(
                    "WARNING: {} is not a child of {}; ignoring",
                    p.display(),
                    &args.dir
                );
            }
            continue;
        }

        let rel_path = rel_path.unwrap();

        if args.verbose >= LV_INFO {
            eprintln!(" -> {}", rel_path.display());
        }
        if !args.dry_run {
            // TODO: rel_path includes package - remove that. Write softlink.
            soft_link(&p.to_path_buf(), &target_path.join(rel_path).to_path_buf());
            println!("{} -> {}", p.display(), target_path.join(rel_path).to_path_buf().display());
        }
    }
}
