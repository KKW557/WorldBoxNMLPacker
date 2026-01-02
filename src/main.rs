use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use serde::Deserialize;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use zip::write::SimpleFileOptions;

#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Asset directories to be included in the package.
    #[arg(long, default_values = &["assets"], help = "Asset directories to be included in the package"
    )]
    assets: Vec<String>,

    /// The command used to build the project
    #[arg(
        long,
        default_value = "dotnet build",
        help = "The command used to build the project"
    )]
    build: String,

    /// Whether to build binary.
    #[arg(short, long, help = "Whether to build binary")]
    compile: bool,

    /// Additional files or directories to include.
    /// Default values are provided for forward compatibility with existing mod structures.
    #[arg(long, default_values = &["Locals", "LICENSE", "default_config.json", "icon.png", "mod.json"], help = "Additional files or directories to include")]
    include: Vec<String>,

    /// The final output path of the packed zip file.
    /// If not specified, it defaults to the 'bin/Mod/<name>-<version>.zip'.
    #[arg(short, long, help = "The final output path of the packed zip file")]
    output: Option<String>,

    /// Whether to include PDB files.
    #[arg(long, default_value_t = true, help = "Whether to include PDB files")]
    pdb: bool,

    /// Source code directories.
    /// Default values are provided for compatibility with various project layouts.
    #[arg(long, default_values = &["Code", "code", "src"], help = "Source code directories")]
    sources: Vec<String>,
}

#[derive(Deserialize)]
struct Mod {
    name: String,
    version: String,
}

struct File {
    pub source: PathBuf,
    pub target: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut files = Vec::new();

    collect_assets_and_include(&cli.assets, &cli.include, &mut files)?;

    let output = generate_output_path(&cli.output, &files)?;

    if cli.compile {
        compile(&cli.build, cli.pdb, &mut files)?;
    } else {
        collect_sources(&cli.sources, &mut files)?;
    }

    zip(&output, &files)?;

    print_packed_message(&output)?;

    Ok(())
}

fn find_file(files: &[File], name: &str) -> Option<PathBuf> {
    files
        .iter()
        .filter(|file| file.source.exists())
        .find(|file| file.source.file_name() == Some(std::ffi::OsStr::new(name)))
        .map(|file| file.source.clone())
}

const ARROW: &str = " -> ";

fn get_dotnet_build(line: &str) -> Option<PathBuf> {
    line.contains(ARROW)
        .then(|| line.split(ARROW).last())
        .flatten()
        .map(|part| PathBuf::from(part.trim()))
        .filter(|path| path.exists())
}

fn collect_files<F>(current: &Path, base: &Path, files: &mut Vec<File>, filter: F) -> Result<()>
where
    F: Fn(&Path) -> bool + Copy,
{
    if !current.exists() {
        return Ok(());
    }

    let metadata = fs::symlink_metadata(current)?;

    if metadata.is_dir() {
        for entry in fs::read_dir(current)? {
            collect_files(&entry?.path(), base, files, filter)?;
        }
    } else if filter(current) {
        let target = current
            .strip_prefix(base)
            .with_context(|| format!("Failed to strip prefix: {}", current.display()))?;

        files.push(File {
            source: current.to_path_buf(),
            target: target.to_path_buf(),
        });
    }

    Ok(())
}

fn collect_assets_and_include(
    assets: &Vec<String>,
    include: &Vec<String>,
    files: &mut Vec<File>,
) -> Result<()> {
    for dir in assets {
        let path = Path::new(dir);
        collect_files(path, path, files, |_| true)?;
    }

    for file in include {
        let source = PathBuf::from(file);
        let target = source.file_name().map(PathBuf::from).unwrap_or_default();
        files.push(File { source, target });
    }

    Ok(())
}

fn generate_output_path(output: &Option<String>, files: &[File]) -> Result<PathBuf> {
    let output = if let Some(output) = output {
        PathBuf::from(output)
    } else {
        let mod_json =
            find_file(files, "mod.json").with_context(|| "Failed to find 'mod.json' in assets")?;

        let content = fs::read_to_string(&mod_json)
            .with_context(|| format!("Failed to read: {}", mod_json.display()))?;

        let mod_struct: Mod = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse: {}", mod_json.display()))?;

        PathBuf::from("bin")
            .join("Mod")
            .join(format!("{}-{}.zip", mod_struct.name, mod_struct.version))
    };

    if let Some(parent) = output.parent() {
        if parent.as_os_str().is_empty() || parent.exists() {
            return Ok(output);
        }
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }
    Ok(output)
}

fn compile(build: &str, pdb: bool, files: &mut Vec<File>) -> Result<()> {
    println!("Compiling with: {}\n", build);

    let parts = shlex::split(build).ok_or_else(|| anyhow!("Invalid build command: {}", build))?;

    if parts.is_empty() {
        bail!("Build command is empty")
    }

    let mut child = std::process::Command::new(&parts[0])
        .args(&parts[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("Failed to execute build command: {}", build))?;

    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);

    let mut count = 0;
    for line in reader.lines() {
        let line = line?;
        println!("{}", line);

        if let Some(source) = get_dotnet_build(&line) {
            let target = source.file_name().map(PathBuf::from).unwrap_or_default();
            files.push(File {
                source: source.clone(),
                target,
            });
            count += 1;
        };
    }

    if pdb {
        let mut pdbs = Vec::new();
        for file in files.iter().skip(files.len() - count) {
            let source = file.source.with_extension("pdb");
            if source.exists() {
                let target = source.file_name().map(PathBuf::from).unwrap_or_default();
                pdbs.push(File { source, target });
            }
        }
        count += pdbs.len();
        files.extend(pdbs);
    }

    println!();

    if count == 0 {
        bail!("No compiled files found");
    } else {
        println!("Compiled {} files", count);
    }

    Ok(())
}

fn collect_sources(sources: &[String], files: &mut Vec<File>) -> Result<()> {
    for source in sources {
        let path = Path::new(source);
        if path.exists() {
            let base = path.parent().unwrap_or_else(|| Path::new("."));
            collect_files(path, base, files, |p| {
                p.extension().is_some_and(|e| e.eq_ignore_ascii_case("cs"))
            })?;
        }
    }

    Ok(())
}

fn zip(path: &PathBuf, files: &[File]) -> Result<()> {
    let file = fs::File::create(path)
        .with_context(|| format!("Failed to create file: {}", path.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    for file in files.iter().filter(|f| f.source.exists()) {
        if file.source.is_dir() {
            continue;
        }

        let path = file.target.to_string_lossy().replace('\\', "/");
        zip.start_file(path, options)?;

        let mut content = fs::File::open(&file.source)
            .with_context(|| format!("Failed to open: {}", file.source.display()))?;
        std::io::copy(&mut content, &mut zip)?;
    }

    zip.finish()?;
    Ok(())
}

fn print_packed_message(output: &PathBuf) -> Result<()> {
    let output = if output.is_absolute() {
        output
    } else {
        &std::env::current_dir()
            .context("Failed to get current directory")?
            .join(output)
    };

    let output = output
        .iter()
        .map(|s| s.to_string_lossy())
        .collect::<Vec<_>>()
        .join(std::path::MAIN_SEPARATOR_STR);

    println!(
        "Packed mod at: \x1b]8;;file://{}\x1b\\{}\x1b]8;;\x1b\\",
        output.replace('\\', "/"),
        output
    );

    Ok(())
}
