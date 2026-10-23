use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

/// Read, write, and query file metadata.
#[derive(Debug, Parser)]
#[command(
    name = "xmeta",
    version,
    about = "Read, write, and query file metadata (native xattr / Windows ADS, bundled SQLite)",
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
    /// Manage typed fields stored on a folder's extended attributes.
    Schema(SchemaArgs),
    /// Query a folder using the bundled SQLite engine and fs_meta extension.
    Query(QueryArgs),
}

#[derive(Debug, Args)]
pub struct SchemaArgs {
    #[command(subcommand)]
    pub command: SchemaCommand,
}

#[derive(Debug, Subcommand)]
pub enum SchemaCommand {
    /// Read the folder schema (an absent schema has no custom fields).
    Get {
        #[arg(default_value = ".", value_name = "PATH")]
        path: PathBuf,
    },
    /// Replace the folder's field declarations; file properties are unchanged.
    Set {
        #[arg(default_value = ".", value_name = "PATH")]
        path: PathBuf,
        /// Comma-separated declarations, e.g. "rating INTEGER, status TEXT".
        #[arg(
            value_name = "FIELDS",
            required_unless_present = "from",
            conflicts_with = "from"
        )]
        fields: Option<String>,
        /// Read a versioned schema JSON object from FILE, or "-" for stdin.
        #[arg(long, value_name = "FILE")]
        from: Option<PathBuf>,
    },
    /// Remove only the folder schema; file metadata is preserved.
    Clear {
        #[arg(default_value = ".", value_name = "PATH")]
        path: PathBuf,
    },
}

#[derive(Debug, Args)]
pub struct QueryArgs {
    /// Root folder; only this folder's schema defines the query columns.
    #[arg(default_value = ".", value_name = "PATH")]
    pub path: PathBuf,
    /// A single read-only SQL query against the "files" table.
    /// Defaults to file paths, sizes, and the folder's custom columns.
    #[arg(value_name = "SQL")]
    pub sql: Option<String>,
    /// Maximum rows returned per page (1–10000).
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(1..=10000))]
    pub limit: u32,
    /// Skip this many rows of the query result. Use ORDER BY for stable pages.
    #[arg(long, default_value_t = 0)]
    pub offset: u64,
    /// JSON array bound to SQL's positional ? parameters.
    #[arg(long, default_value = "[]", value_name = "JSON")]
    pub params: String,
    /// Interrupt SQLite execution after this many milliseconds.
    #[arg(long, default_value_t = 10000, value_parser = clap::value_parser!(u32).range(1..))]
    pub timeout_ms: u32,
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
