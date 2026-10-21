use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

/// Read and write JSON properties stored in extended metadata.
#[derive(Debug, Parser)]
#[command(
    name = "xmeta",
    version,
    about = "Read and write JSON properties stored in extended metadata (xattr / Windows ADS)",
    arg_required_else_help = true,
    subcommand_required = true,
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Override the attribute namespace.
    ///
    /// Defaults to FS_META_NAMESPACE, the --config file, the nearest .xmeta.json,
    /// the user config, or space.eidos.meta, in that order.
    #[arg(short = 'n', long, global = true, value_name = "NAMESPACE")]
    pub namespace: Option<String>,

    /// Use an explicit config file and disable automatic config discovery.
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Emit compact, machine-readable JSON.
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Read all properties, or selected keys.
    Get(GetArgs),
    /// Set properties from KEY=VALUE / KEY:=JSON assignments.
    Set(SetArgs),
    /// Remove property keys.
    Unset(UnsetArgs),
    /// Merge properties from flags and a JSON object.
    Patch(PatchArgs),
    /// Remove the entire property namespace.
    Clear(ClearArgs),
    /// Inspect or update the default namespace configuration.
    Config(ConfigArgs),
}

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: Option<ConfigCommand>,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Print the effective namespace only.
    Get,
    /// Set the default namespace.
    Set {
        /// Namespace to store as the default.
        #[arg(value_name = "NAMESPACE")]
        namespace: String,

        /// Write .xmeta.json in the current project instead of the user config.
        #[arg(long)]
        project: bool,
    },
    /// Remove the configured namespace.
    Unset {
        /// Update .xmeta.json in the current project instead of the user config.
        #[arg(long)]
        project: bool,
    },
    /// Print a config file path.
    Path {
        /// Print the project config path instead of the user config path.
        #[arg(long)]
        project: bool,
    },
}

#[derive(Debug, Args)]
pub struct GetArgs {
    /// Existing file or directory.
    #[arg(value_name = "PATH")]
    pub path: PathBuf,

    /// Property keys to read.
    #[arg(value_name = "KEY")]
    pub keys: Vec<String>,

    /// Print the raw stored bytes. Only valid without KEY.
    #[arg(long)]
    pub raw: bool,

    /// Print scalar values without JSON quoting. Requires KEY.
    #[arg(long)]
    pub text: bool,
}

#[derive(Debug, Args)]
pub struct SetArgs {
    /// Existing file or directory.
    #[arg(value_name = "PATH")]
    pub path: PathBuf,

    /// KEY=VALUE stores a string; KEY:=JSON stores parsed JSON.
    #[arg(value_name = "KEY=VALUE", required = true)]
    pub assignments: Vec<String>,

    /// Replace the namespace instead of merging into it.
    #[arg(long)]
    pub replace: bool,

    /// Parse every VALUE as JSON.
    #[arg(long)]
    pub json_values: bool,

    /// Print the result without writing it.
    #[arg(long)]
    pub dry_run: bool,

    /// Print the resulting object after a successful write.
    #[arg(long)]
    pub print: bool,
}

#[derive(Debug, Args)]
pub struct UnsetArgs {
    /// Existing file or directory.
    #[arg(value_name = "PATH")]
    pub path: PathBuf,

    /// Property keys to remove.
    #[arg(value_name = "KEY", required = true)]
    pub keys: Vec<String>,

    /// Fail when a key does not exist.
    #[arg(long)]
    pub strict: bool,

    /// Print the result without writing it.
    #[arg(long)]
    pub dry_run: bool,

    /// Print the resulting object after a successful write.
    #[arg(long)]
    pub print: bool,
}

#[derive(Debug, Args)]
pub struct PatchArgs {
    /// Existing file or directory.
    #[arg(value_name = "PATH")]
    pub path: PathBuf,

    /// KEY=VALUE stores a string; KEY:=JSON stores parsed JSON.
    #[arg(long = "set", value_name = "KEY=VALUE")]
    pub sets: Vec<String>,

    /// Property key to remove.
    #[arg(long = "remove", value_name = "KEY")]
    pub removes: Vec<String>,

    /// Read a JSON object from FILE; use "-" for stdin.
    #[arg(long = "from", value_name = "FILE")]
    pub from: Option<PathBuf>,

    /// Replace the namespace instead of merging into it.
    #[arg(long)]
    pub replace: bool,

    /// Parse every --set VALUE as JSON.
    #[arg(long)]
    pub json_values: bool,

    /// Print the result without writing it.
    #[arg(long)]
    pub dry_run: bool,

    /// Print the resulting object after a successful write.
    #[arg(long)]
    pub print: bool,
}

#[derive(Debug, Args)]
pub struct ClearArgs {
    /// Existing file or directory.
    #[arg(value_name = "PATH")]
    pub path: PathBuf,
}
