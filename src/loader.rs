use crate::Error;
use std::fmt::{Debug, Formatter};
use std::fs;
use std::path::PathBuf;
use steamlocate::SteamDir;
use tracing::{debug, info};
use vbsp::Packfile;
use vpk::VPK;

pub struct Loader {
    pack: Option<Packfile>,
    tf_dir: PathBuf,
    download: PathBuf,
    vpks: Vec<VPK>,
}

impl Debug for Loader {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Loader")
            .field("tf_dir", &self.tf_dir)
            .finish_non_exhaustive()
    }
}

impl Loader {
    pub fn new() -> Result<Self, Error> {
        let tf_dir = SteamDir::locate()
            .ok_or("Can't find steam directory")?
            .app(&440)
            .ok_or("Can't find tf2 directory")?
            .path
            .join("tf");
        let download = tf_dir.join("download");
        let vpks = tf_dir
            .read_dir()?
            .filter_map(|item| item.ok())
            .filter_map(|item| Some(item.path().to_str()?.to_string()))
            .filter(|path| path.ends_with("dir.vpk"))
            .map(|path| vpk::from_path(&path))
            .filter_map(|res| res.ok())
            .collect();

        Ok(Loader {
            tf_dir,
            download,
            vpks,
            pack: None,
        })
    }

    pub fn set_pack(&mut self, pack: Packfile) {
        self.pack = Some(pack);
    }

    #[tracing::instrument]
    pub fn exists(&self, name: &str) -> bool {
        debug!("loading {}", name);
        if name.ends_with("bsp") {
            let path = self.tf_dir.join(name);
            if path.exists() {
                return true;
            }
            let path = self.download.join(name);
            if path.exists() {
                return true;
            }
        }
        for vpk in self.vpks.iter() {
            if vpk.tree.contains_key(name) {
                return true;
            }
        }
        if let Some(pack) = &self.pack {
            if let Some(_) = pack.get(name).ok().flatten() {
                return true;
            }
        }
        false
    }

    #[tracing::instrument]
    pub fn load(&self, name: &str) -> Result<Vec<u8>, Error> {
        debug!("loading {}", name);
        if name.ends_with("bsp") {
            let path = self.tf_dir.join(name);
            if path.exists() {
                debug!("found in tf2 dir");
                return Ok(fs::read(path)?);
            }
            let path = self.download.join(name);
            if path.exists() {
                debug!("found in download dir");
                return Ok(fs::read(path)?);
            }
        }
        if let Some(pack) = &self.pack {
            if let Some(data) = pack.get(name)? {
                debug!("got {} bytes from packfile", data.len());
                return Ok(data);
            }
        }
        for vpk in self.vpks.iter() {
            if let Some(entry) = vpk.tree.get(name) {
                let data = entry.get()?.into_owned();
                debug!("got {} bytes from vpk", data.len());
                return Ok(data);
            }
        }
        info!("Failed to find {} in vpk", name);
        Err(Error::ResourceNotFound(name.to_string()))
    }

    pub fn find_in_paths(&self, name: &str, paths: &[String]) -> Option<String> {
        for path in paths {
            let full_path = format!("{}{}", path, name);
            if self.exists(&full_path) {
                return Some(full_path);
            }
        }
        None
    }
}
