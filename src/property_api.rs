//! Namespace-level property operations independent of virtual-table schemas.
use serde_json::Value;
use sqlite_loadable::{api, define_scalar_function, ext::sqlite3, prelude::*, Error, Result};
use std::{collections::HashMap, fs::File, path::Path, sync::Mutex};

static OPERATIONS: Mutex<()> = Mutex::new(());
const LIMIT: usize = 1024 * 1024;
fn invalid(message: impl ToString) -> Error {
    Error::new_message(message.to_string())
}
fn namespace(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 255
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    {
        return Err(invalid("Invalid property namespace"));
    }
    Ok(())
}
fn open(path: &Path, writing: bool) -> Result<File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0x1 | 0x2).custom_flags(0x00200000);
    }
    let file = options.open(path).map_err(invalid)?;
    let metadata = file.metadata().map_err(invalid)?;
    if !metadata.is_file() {
        return Err(invalid("Properties require a regular file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if writing && metadata.nlink() != 1 {
            return Err(invalid("Property writes reject files with multiple links"));
        }
    }
    #[cfg(not(unix))]
    let _ = writing;
    Ok(file)
}
fn read(file: &File, path: &Path, ns: &str) -> Result<HashMap<String, Value>> {
    #[cfg(unix)]
    let bytes = {
        use xattr::FileExt;
        let name = if cfg!(target_os = "linux") && !ns.starts_with("user.") {
            format!("user.{ns}")
        } else {
            ns.to_owned()
        };
        file.get_xattr(name).map_err(invalid)?
    };
    #[cfg(not(unix))]
    let bytes = crate::meta::read_envelope(path, ns).map_err(invalid)?;
    let _ = (file, path);
    match bytes {
        None => Ok(HashMap::new()),
        Some(bytes) if bytes.len() <= LIMIT => serde_json::from_slice(&bytes).map_err(invalid),
        Some(_) => Err(invalid("Property namespace exceeds 1 MiB")),
    }
}
fn write(file: &File, path: &Path, ns: &str, values: &HashMap<String, Value>) -> Result<String> {
    let text = serde_json::to_string(values).map_err(invalid)?;
    if text.len() > LIMIT {
        return Err(invalid("Property namespace exceeds 1 MiB"));
    }
    #[cfg(unix)]
    {
        use xattr::FileExt;
        let name = if cfg!(target_os = "linux") && !ns.starts_with("user.") {
            format!("user.{ns}")
        } else {
            ns.to_owned()
        };
        file.set_xattr(name, text.as_bytes()).map_err(invalid)?;
    }
    #[cfg(not(unix))]
    crate::meta::restore_envelope(path, ns, Some(text.as_bytes())).map_err(invalid)?;
    let _ = (file, path);
    Ok(text)
}
pub fn register(db: *mut sqlite3) -> Result<()> {
    define_scalar_function(
        db,
        "fs_meta_read",
        2,
        |context, args| {
            let path = Path::new(api::value_text_notnull(&args[0])?);
            let ns = api::value_text_notnull(&args[1])?;
            namespace(ns)?;
            let _guard = OPERATIONS.lock().map_err(invalid)?;
            let file = open(path, false)?;
            api::result_text(
                context,
                serde_json::to_string(&read(&file, path, ns)?).map_err(invalid)?,
            )
        },
        FunctionFlags::UTF8 | FunctionFlags::DIRECTONLY,
    )?;
    define_scalar_function(
        db,
        "fs_meta_patch",
        4,
        |context, args| {
            let path = Path::new(api::value_text_notnull(&args[0])?);
            let ns = api::value_text_notnull(&args[1])?;
            namespace(ns)?;
            let set: HashMap<String, Value> =
                serde_json::from_str(api::value_text_notnull(&args[2])?).map_err(invalid)?;
            let remove: Vec<String> =
                serde_json::from_str(api::value_text_notnull(&args[3])?).map_err(invalid)?;
            if set
                .keys()
                .chain(remove.iter())
                .any(|key| key.is_empty() || key.len() > 1024 || key.contains('\0'))
            {
                return Err(invalid("Invalid property key"));
            }
            if remove.iter().any(|key| set.contains_key(key)) {
                return Err(invalid("Cannot set and remove the same property"));
            }
            let _guard = OPERATIONS.lock().map_err(invalid)?;
            let file = open(path, true)?;
            let mut values = read(&file, path, ns)?;
            for key in remove {
                values.remove(&key);
            }
            values.extend(set);
            api::result_text(context, write(&file, path, ns, &values)?)
        },
        FunctionFlags::UTF8 | FunctionFlags::DIRECTONLY,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_corrupt_envelopes_without_replacing_them() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("note.txt");
        std::fs::write(&path, "content").unwrap();
        crate::meta::restore_envelope(&path, "space.eidos.test", Some(b"corrupt")).unwrap();
        let file = open(&path, false).unwrap();
        assert!(read(&file, &path, "space.eidos.test").is_err());
        assert_eq!(
            crate::meta::read_envelope(&path, "space.eidos.test").unwrap(),
            Some(b"corrupt".to_vec())
        );
        assert!(namespace("../escape").is_err());
        assert!(open(temp.path(), false).is_err());
    }
}
