use clap::Parser;
use std::env;
use std::io;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::str;
use walkdir::WalkDir;

const LV_WARN: u8 = 1;
const LV_INFO: u8 = 2;
const LV_DEBUG: u8 = 3;

#[derive(clap::ValueEnum, Clone, Debug)]
enum ConflictResolution {
    /// Immediately stop running ranch.
    Stop,
    /// Ignore the existing file; continue soft-linking the remaining files.
    Ignore,
    /// Deletes the existing file, replacing it with the soft link.
    Overwrite,
    /// Overwrites the source file with the contents of the existing file, then
    /// replaces the existing file with a soft link.
    Adopt,
    /// Ranch stops running, instead removing all previously created soft-links.
    Rollback,
}

fn parse_dir(s: &str) -> Result<String, String> {
    if s == "." {
        return Ok(env::var("RANCH_DIR").unwrap_or_else(|_| {
            env::current_dir()
                .expect("FATAL: Could not open the current directory.")
                .into_os_string()
                .into_string()
                .expect("FATAL: Could not decode 'dir'; invalid unicode encountered.")
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
Consider the following example in '/home/alice':

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
    /// Do not perform any operations that modify the filesystem; merely show what would happen
    #[arg(
        short = 'n',
        long,
        alias = "no",
        default_value_t = false,
    )]
    dry_run: bool,

    /// Change directory to 'DIR' to search for packages instead of using the current directory
    #[arg(
        short = 'C',
        long,
        default_value = ".",
        value_parser = parse_dir,
    )]
    dir: String,

    /// Destination directory where symlinks are deployed; default implies 'DIR/..'
    #[arg(
        short = 't',
        long,
    )]
    target: Option<String>,

    /// Standard error output verbosity (nothing by default); specify multiple times to print more
    #[arg(
        short = 'v',
        long,
        action = clap::ArgAction::Count,
    )]
    verbose: u8,

    /// Deletes the package from the target dir; only symlinks are deleted
    #[arg(
        short = 'D',
        long,
        default_value = "",
    )]
    delete: String,

    /// Determines what ranch should do if it finds an existing file where a softlink will be
    /// created
    #[arg(
        short = 'e',
        value_enum,
        long,
        default_value_t=ConflictResolution::Stop,
    )]
    exists: ConflictResolution,

    /// Name of a subdirectory of 'DIR' containing files to symlink
    #[arg()]
    package: String,
}

#[cfg(windows)]
fn soft_link(from: &Path, to: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_file(from, to)
}

#[cfg(unix)]
fn soft_link(from: &Path, to: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(from, to)
}

fn exec(argv: &[String], stderr: &mut impl io::Write) {
    let args = Args::parse_from(argv);
    if args.verbose >= LV_DEBUG {
        _ = writeln!(stderr, "{:?}", &args);
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
        _ = writeln!(
            stderr,
            "Linking... {} => {}",
            &prefix_path.display(),
            &target_path.display()
        );
    }
    if !prefix_path.exists() {
        _ = writeln!(
            stderr,
            "FATAL: Package {} does not exist; exiting now",
            args.package
        );
        exit(1);
    }

    // Check destination path
    std::fs::create_dir_all(&target_path).expect("FATAL: Could not create target directory");

    // Make links
    for src in WalkDir::new(prefix_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|r| r.ok())
        .map(|e| e.path().to_path_buf())
    {
        if !(src.is_file() || src.is_symlink()) {
            continue;
        }

        let rel_path = src.strip_prefix(&args.dir);
        if rel_path.is_err() {
            if args.verbose >= LV_WARN {
                _ = writeln!(
                    stderr,
                    "WARNING: {} is not a child of {}; ignoring",
                    src.display(),
                    &args.dir
                );
            }
            continue;
        }

        let rel_path = rel_path.unwrap();

        let relative_output = rel_path.strip_prefix(&args.package).unwrap();
        let output_path = target_path.join(relative_output).to_path_buf();
        if args.verbose >= LV_INFO {
            _ = writeln!(
                stderr,
                "{} -> {}",
                &src.to_path_buf().display(),
                &output_path.display()
            );
        }
        if !args.dry_run {
            soft_link(&src.to_path_buf(), &output_path).unwrap();
        }
    }
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    exec(&argv, &mut io::stderr());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{File,create_dir_all};
    use tempdir::TempDir;

    fn make_dummy_fs(dir: &Path)
    {
        let dotfiles_home = dir.join(".dotfiles/home");
        create_dir_all(&dotfiles_home).unwrap();

        File::create(dotfiles_home.join(".vimrc")).unwrap();
    }

    #[test]
    fn test_example()
    {
        println!("GIVEN");
        let mut stderr = io::BufWriter::new(Vec::new());
        let tmp_dir = TempDir::new("alice").unwrap();
        make_dummy_fs(tmp_dir.path());

        println!("WHEN");
        exec(&[
            "ranch",
            "-vvv",
            "-C",
            tmp_dir.path().join(".dotfiles").to_str().unwrap(),
            "home"
        ].map(|s| s.to_owned()), &mut stderr);

        println!("THEN");
        let bytes = stderr.into_inner().unwrap();
        let string = String::from_utf8(bytes).unwrap();
        println!("{}", string);
    }
}
