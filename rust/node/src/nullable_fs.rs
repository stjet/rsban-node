use std::{
    collections::{HashMap, HashSet},
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Arc,
};

use rsnano_core::utils::{OutputListenerMt, OutputTrackerMt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FsEvent {
    event_type: EventType,
    path: PathBuf,
    contents: String,
}

impl FsEvent {
    pub fn create_dir_all(path: impl Into<PathBuf>) -> Self {
        Self {
            event_type: EventType::CreateDirAll,
            path: path.into(),
            contents: String::new(),
        }
    }

    pub fn write(path: impl Into<PathBuf>, contents: impl AsRef<[u8]>) -> Self {
        Self {
            event_type: EventType::Write,
            path: path.into(),
            contents: String::from_utf8_lossy(contents.as_ref()).to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventType {
    CreateDirAll,
    Write,
}

pub(crate) struct NullableFilesystem {
    fs: Box<dyn Filesystem>,
    listener: OutputListenerMt<FsEvent>,
}

impl NullableFilesystem {
    pub fn new() -> Self {
        Self {
            fs: Box::new(RealFilesystem {}),
            listener: OutputListenerMt::new(),
        }
    }

    pub fn new_null() -> Self {
        Self {
            fs: Box::new(FilesystemStub::default()),
            listener: OutputListenerMt::new(),
        }
    }

    pub fn null_builder() -> NullableFilesystemBuilder {
        NullableFilesystemBuilder {
            stub: FilesystemStub::default(),
        }
    }

    pub fn exists(&self, f: impl AsRef<Path>) -> bool {
        self.fs.exists(f.as_ref())
    }

    pub fn read_to_string(&mut self, f: impl AsRef<Path>) -> std::io::Result<String> {
        self.fs.read_to_string(f.as_ref())
    }

    pub fn create_dir_all(&self, f: impl AsRef<Path>) -> std::io::Result<()> {
        let path = f.as_ref();
        self.listener.emit(FsEvent::create_dir_all(path));
        self.fs.create_dir_all(path)
    }

    pub fn write(&self, path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> std::io::Result<()> {
        let path = path.as_ref();
        let contents = contents.as_ref();
        self.listener.emit(FsEvent::write(path, contents));
        self.fs.write(path, contents)
    }

    pub fn track(&self) -> Arc<OutputTrackerMt<FsEvent>> {
        self.listener.track()
    }
}

impl Default for NullableFilesystem {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) struct NullableFilesystemBuilder {
    stub: FilesystemStub,
}

impl NullableFilesystemBuilder {
    pub fn path_exists(mut self, path: impl Into<PathBuf>) -> Self {
        self.stub.exists.insert(path.into());
        self
    }

    pub fn read_to_string(mut self, path: impl Into<PathBuf>, contents: String) -> Self {
        self.stub.read_to_string.insert(path.into(), Ok(contents));
        self
    }

    pub fn read_to_string_fails(mut self, path: impl Into<PathBuf>, error: std::io::Error) -> Self {
        self.stub.read_to_string.insert(path.into(), Err(error));
        self
    }

    pub fn finish(self) -> NullableFilesystem {
        NullableFilesystem {
            fs: Box::new(self.stub),
            listener: OutputListenerMt::new(),
        }
    }
}

trait Filesystem {
    fn exists(&self, path: &Path) -> bool;
    fn read_to_string(&mut self, f: &Path) -> std::io::Result<String>;
    fn create_dir_all(&self, path: &Path) -> std::io::Result<()>;
    fn write(&self, path: &Path, contents: &[u8]) -> std::io::Result<()>;
}

struct RealFilesystem {}

impl Filesystem for RealFilesystem {
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn read_to_string(&mut self, f: &Path) -> std::io::Result<String> {
        std::fs::read_to_string(f)
    }

    fn create_dir_all(&self, path: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn write(&self, path: &Path, contents: &[u8]) -> std::io::Result<()> {
        std::fs::write(path, contents)
    }
}

#[derive(Default)]
struct FilesystemStub {
    exists: HashSet<PathBuf>,
    read_to_string: HashMap<PathBuf, std::io::Result<String>>,
}

impl Filesystem for FilesystemStub {
    fn exists(&self, path: &Path) -> bool {
        self.exists.contains(path)
    }

    fn read_to_string(&mut self, f: &Path) -> std::io::Result<String> {
        match self.read_to_string.remove(f) {
            Some(contents) => contents,
            None => Err(std::io::Error::new(
                ErrorKind::NotFound,
                format!("no response configured for file {f:?}"),
            )),
        }
    }

    fn create_dir_all(&self, _path: &Path) -> std::io::Result<()> {
        Ok(())
    }

    fn write(&self, _path: &Path, _contents: &[u8]) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_exists() {
        let path: PathBuf = "/tmp/nullable-fs-test.txt".into();
        if path.exists() {
            std::fs::remove_file(&path).unwrap();
        }

        let fs = NullableFilesystem::new();
        assert_eq!(fs.exists(&path), false);

        std::fs::write(&path, b"test").unwrap();
        assert_eq!(fs.exists(&path), true);

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn read_to_string() {
        let path: PathBuf = "/tmp/nullable-fs-read-to-string.txt".into();
        std::fs::write(&path, b"hello world").unwrap();
        let result = NullableFilesystem::new().read_to_string(&path);
        std::fs::remove_file(path).unwrap();
        assert_eq!(result.unwrap(), "hello world")
    }

    #[test]
    fn create_dir_all() {
        let p = PathBuf::from("/tmp/a");
        if p.exists() {
            std::fs::remove_dir_all(&p).unwrap();
        }

        NullableFilesystem::new()
            .create_dir_all("/tmp/a/b/c")
            .unwrap();

        assert!(PathBuf::from("/tmp/a/b/c").exists());
        std::fs::remove_dir_all(p).unwrap()
    }

    #[test]
    fn write() {
        let f = PathBuf::from("/tmp/nullable-fs-write-test.txt");
        NullableFilesystem::new().write(&f, b"foo").unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "foo");
        std::fs::remove_file(f).unwrap();
    }

    mod observability {
        use super::*;

        #[test]
        fn create_dir_all_can_be_tracked() {
            let fs = NullableFilesystem::new_null();
            let tracker = fs.track();
            let path = PathBuf::from("/foo/bar");
            fs.create_dir_all(&path).unwrap();
            let output = tracker.output();
            assert_eq!(output.len(), 1);
            assert_eq!(output[0].event_type, EventType::CreateDirAll);
            assert_eq!(output[0].path, path);
        }

        #[test]
        fn write_can_be_tracked() {
            let fs = NullableFilesystem::new_null();
            let tracker = fs.track();
            let path = PathBuf::from("/foo/bar");
            fs.write(&path, b"hello").unwrap();
            let output = tracker.output();
            assert_eq!(output.len(), 1);
            assert_eq!(output[0].event_type, EventType::Write);
            assert_eq!(output[0].path, path);
            assert_eq!(output[0].contents, "hello");
        }
    }

    mod nullability {
        use super::*;

        #[test]
        fn is_nullable() {
            let mut fs = NullableFilesystem::new_null();
            assert_eq!(fs.exists("/foo/bar"), false);
            assert!(fs.read_to_string("/foo/bar").is_err());
            assert!(fs.create_dir_all("/foo/bar").is_ok());
            assert!(fs.write("/foo/bar", "abc").is_ok());
        }

        #[test]
        fn file_exists() {
            let fs = NullableFilesystem::null_builder()
                .path_exists("/foo/bar")
                .finish();
            assert_eq!(fs.exists("/foo/bar"), true);
            assert_eq!(fs.exists("/foo/bar"), true);
            assert_eq!(fs.exists("/foo/bar2"), false);
        }

        #[test]
        fn read_to_string_file_not_found() {
            let mut fs = NullableFilesystem::new_null();
            let err = fs.read_to_string("/foo/bar").unwrap_err();
            assert_eq!(err.kind(), ErrorKind::NotFound);
            assert_eq!(
                err.to_string(),
                "no response configured for file \"/foo/bar\""
            );
        }

        #[test]
        fn read_to_string() {
            let path = PathBuf::from("/foo/bar");
            let mut fs = NullableFilesystem::null_builder()
                .read_to_string(&path, "hello world".to_string())
                .finish();

            assert_eq!(fs.read_to_string(path).unwrap(), "hello world");
        }

        #[test]
        fn read_to_string_fails() {
            let path = PathBuf::from("/foo/bar");
            let mut fs = NullableFilesystem::null_builder()
                .read_to_string_fails(&path, std::io::Error::new(ErrorKind::PermissionDenied, ""))
                .finish();

            let err = fs.read_to_string(path).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::PermissionDenied);
        }
    }
}
