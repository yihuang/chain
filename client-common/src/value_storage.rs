//!
use parity_scale_codec::{Decode, Encode};
use secstr::SecUtf8;

use crate::storage::{SecureStorage, Storage};
use crate::{Error, ErrorKind, Result, ResultExt};

/// Data type can store with storage
pub trait StorageValueType {
    /// name of data type, will appear in error messages
    fn name() -> &'static str;
    /// keyspace for this value type
    fn keyspace() -> &'static str;
}

/// Value extension trait for `Storage`
pub trait ValueStorage {
    /// Load value
    fn get_value<T: StorageValueType + Decode>(&self, name: &str) -> Result<Option<T>>;
    /// Save value
    fn set_value<T: StorageValueType + Encode>(&self, name: &str, value: &T) -> Result<()>;
    /// Delete value
    fn delete_value<T: StorageValueType>(&self, name: &str) -> Result<()>;
    /// Clear all values
    fn clear_values<T: StorageValueType>(&self) -> Result<()>;
}

/// Value extension trait for `SecureStorage`
pub trait SecureValueStorage: ValueStorage {
    /// Load and deserialize object
    fn get_value_secure<T: StorageValueType + Decode>(
        &self,
        name: &str,
        passphrase: &SecUtf8,
    ) -> Result<Option<T>>;
    /// Serialize and save value
    fn set_value_secure<T: StorageValueType + Encode>(
        &self,
        name: &str,
        value: &T,
        passphrase: &SecUtf8,
    ) -> Result<()>;
    /// Like `set_value_secure`, error if key already exists.
    fn create_value_secure<T: StorageValueType + Encode>(
        &self,
        key: &str,
        value: &T,
        passphrase: &SecUtf8,
    ) -> Result<()>;
    /// Modify secure value
    fn modify_value_secure<T, F>(&self, name: &str, passphrase: &SecUtf8, f: F) -> Result<()>
    where
        T: StorageValueType + Encode + Decode + Default,
        F: Fn(&mut T) -> Result<()>;
    /// Modify secure value, error if not exists.
    fn modify_value_secure_strict<T, F>(
        &self,
        name: &str,
        passphrase: &SecUtf8,
        f: F,
    ) -> Result<()>
    where
        T: StorageValueType + Encode + Decode,
        F: Fn(&mut T) -> Result<()>;
}

impl<S> ValueStorage for S
where
    S: Storage,
{
    /// load and deserialize object
    fn get_value<T: StorageValueType + Decode>(&self, key: &str) -> Result<Option<T>> {
        if let Some(bytes) = self.get(T::keyspace(), key)? {
            Ok(Some(
                T::decode(&mut bytes.as_slice())
                    .err_kind(ErrorKind::DeserializationError, || {
                        format!("decode: {}/{}", T::keyspace(), key)
                    })?,
            ))
        } else {
            Ok(None)
        }
    }

    /// serialize and save object
    fn set_value<T: StorageValueType + Encode>(&self, key: &str, value: &T) -> Result<()> {
        self.set(T::keyspace(), key, value.encode()).map(|_| ())
    }

    fn delete_value<T: StorageValueType>(&self, name: &str) -> Result<()> {
        self.delete(T::keyspace(), name)?;
        Ok(())
    }
    fn clear_values<T: StorageValueType>(&self) -> Result<()> {
        self.clear(T::keyspace())
    }
}

impl<S> SecureValueStorage for S
where
    S: SecureStorage,
{
    fn get_value_secure<T: StorageValueType + Decode>(
        &self,
        key: &str,
        passphrase: &SecUtf8,
    ) -> Result<Option<T>> {
        if let Some(bytes) = self.get_secure(T::keyspace(), key, passphrase)? {
            Ok(Some(
                T::decode(&mut bytes.as_slice())
                    .err_kind(ErrorKind::DeserializationError, || {
                        format!("decode: {}/{}", T::keyspace(), key)
                    })?,
            ))
        } else {
            Ok(None)
        }
    }

    fn set_value_secure<T: StorageValueType + Encode>(
        &self,
        key: &str,
        value: &T,
        passphrase: &SecUtf8,
    ) -> Result<()> {
        self.set_secure(T::keyspace(), key, value.encode(), passphrase)
            .map(|_| ())
    }

    fn create_value_secure<T: StorageValueType + Encode>(
        &self,
        key: &str,
        value: &T,
        passphrase: &SecUtf8,
    ) -> Result<()> {
        if self.contains_key(T::keyspace(), key)? {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Create duplicate value",
            ));
        }
        self.set_value_secure(key, value, passphrase)
    }

    fn modify_value_secure_strict<T, F>(&self, name: &str, passphrase: &SecUtf8, f: F) -> Result<()>
    where
        T: StorageValueType + Encode + Decode,
        F: Fn(&mut T) -> Result<()>,
    {
        self.fetch_and_update_secure(T::keyspace(), name, passphrase, |bytes_optional| {
            let mut bytes = bytes_optional.err_kind(ErrorKind::InvalidInput, || {
                format!("{} named {} is not found", T::name(), name)
            })?;
            let mut value = T::decode(&mut bytes)
                .err_kind(ErrorKind::DeserializationError, || {
                    format!("Unable to deserialize {} for name {}", T::name(), name)
                })?;

            f(&mut value)?;
            Ok(Some(value.encode()))
        })?;
        Ok(())
    }

    fn modify_value_secure<T, F>(&self, name: &str, passphrase: &SecUtf8, f: F) -> Result<()>
    where
        T: StorageValueType + Encode + Decode + Default,
        F: Fn(&mut T) -> Result<()>,
    {
        self.fetch_and_update_secure(T::keyspace(), name, passphrase, |bytes_optional| {
            let mut value = bytes_optional
                .map(|mut bytes| {
                    T::decode(&mut bytes).err_kind(ErrorKind::DeserializationError, || {
                        format!("Unable to deserialize {} for name {}", T::name(), name)
                    })
                })
                .transpose()?
                .unwrap_or_default();

            f(&mut value)?;
            Ok(Some(value.encode()))
        })?;
        Ok(())
    }
}
