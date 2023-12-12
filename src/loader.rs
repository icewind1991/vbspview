use crate::Error;
use std::env::var;
use std::fmt::{Debug, Formatter};
use std::fs;
use std::path::PathBuf;
use steamlocate::SteamDir;
use tracing::{debug, error, info};
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
        Self::with_opt_pack(None)
    }

    #[allow(dead_code)]
    pub fn with_pak(pack: Packfile) -> Result<Self, Error> {
        Self::with_opt_pack(Some(pack))
    }

    pub fn with_opt_pack(pack: Option<Packfile>) -> Result<Self, Error> {
        let tf2_dir = match var("TF_DIR") {
            Ok(dir) => PathBuf::from(dir),
            Err(_) => SteamDir::locate()
                .ok_or("Can't find steam directory")?
                .app(&440)
                .ok_or("Can't find tf2 directory")?
                .path
                .clone(),
        };
        let tf_dir = tf2_dir.join("tf");
        let vpk_dirs = [tf_dir.clone(), tf2_dir.join("hl2")];

        let download = tf_dir.join("download");
        let vpks = vpk_dirs
            .iter()
            .flat_map(|dir| dir.read_dir())
            .flatten()
            .flatten()
            .filter_map(|item| Some(item.path().to_str()?.to_string()))
            .filter(|path| path.ends_with("dir.vpk"))
            .map(|path| vpk::from_path(&path))
            .filter_map(|res| res.ok())
            .collect();

        Ok(Loader {
            pack,
            tf_dir,
            download,
            vpks,
        })
    }

    pub fn set_pack(&mut self, pack: Packfile) {
        self.pack = Some(pack);
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

    pub fn load_from_paths(&self, name: &str, paths: &[String]) -> Result<Vec<u8>, Error> {
        for path in paths {
            if let Ok(data) = self.load(&format!("{}{}", path, name)) {
                return Ok(data);
            }
        }
        error!("Failed to find {} in vpk paths: {}", name, paths.join(", "));
        Err(Error::ResourceNotFound(name.to_string()))
    }
}
