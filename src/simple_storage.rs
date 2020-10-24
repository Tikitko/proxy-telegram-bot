use std::sync::{Arc, RwLock, Mutex, RwLockWriteGuard, LockResult, RwLockReadGuard};
use std::fs::File;
use std::io::{Write, Read, Error as IoError, Seek, SeekFrom};
pub struct SimpleStorage<V> where
    V: From<String>,
    V: Into<String>,
    V: Clone {
    mem_storage: Arc<RwLock<V>>,
    file_storage: Arc<Mutex<File>>
}

impl<V> Clone for SimpleStorage<V> where
    V: From<String>,
    V: Into<String>,
    V: Clone {
    fn clone(&self) -> Self {
        SimpleStorage {
            mem_storage: self.mem_storage.clone(),
            file_storage: self.file_storage.clone()
        }
    }
}

impl<V> SimpleStorage<V> where
    V: From<String>,
    V: Into<String>,
    V: Clone,
    V: Default {
    pub fn new(file_storage: File) -> Self {
        SimpleStorage {
            mem_storage: Arc::new(Default::default()),
            file_storage: Arc::new(Mutex::new(file_storage))
        }
    }
}

#[derive(Debug)]
pub enum SyncError {
    IoError(IoError),
    PoisonError
}

impl<V> SimpleStorage<V> where
    V: From<String>,
    V: Into<String>,
    V: Clone {
    pub fn mutable_mem_storage(&self) -> LockResult<RwLockWriteGuard<'_, V>> {
        self.mem_storage
            .write()
    }
    pub fn mem_storage(&self) -> LockResult<RwLockReadGuard<'_, V>> {
        self.mem_storage
            .read()
    }
    pub fn sync_mem_from_file(&self) -> Result<(), SyncError> {
        let mut contents = String::new();
        let mut file = self.file_storage
            .lock()
            .map_err(|_| SyncError::PoisonError)?;
        file
            .seek(SeekFrom::Start(0))
            .map_err(|e| SyncError::IoError(e))?;
        file
            .read_to_string(&mut contents)
            .map_err(|e| SyncError::IoError(e))?;
        file
            .flush()
            .map_err(|e| SyncError::IoError(e))?;
        self.mem_storage.write()
            .map(|mut v| *v = V::from(contents))
            .map_err(|_| SyncError::PoisonError)?;
        Ok(())
    }
    pub fn sync_file_from_mem(&self) -> Result<(), SyncError> {
        let contents = self.mem_storage.read()
            .map(|v| (*v).clone().into())
            .map_err(|_| SyncError::PoisonError)?;
        let mut file = self.file_storage
            .lock()
            .map_err(|_| SyncError::PoisonError)?;
        file
            .seek(SeekFrom::Start(0))
            .map_err(|e| SyncError::IoError(e))?;
        file
            .set_len(0)
            .map_err(|e| SyncError::IoError(e))?;
        file
            .write_all(contents.as_ref())
            .map_err(|e| SyncError::IoError(e))?;
        file
            .flush()
            .map_err(|e| SyncError::IoError(e))?;
        Ok(())
    }
}
