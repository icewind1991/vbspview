use crate::Error;
use std::fmt::{Debug, Formatter};
use std::path::PathBuf;
use steamlocate::SteamDir;
use tracing::{debug, error};
use vbsp::Packfile;
use vpk::VPK;

pub struct Loader {
    pack: Packfile,
    tf_dir: PathBuf,
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
    pub fn new(pack: Packfile) -> Result<Self, Error> {
        let tf_dir = SteamDir::locate()
            .ok_or("Can't find steam directory")?
            .app(&440)
            .ok_or("Can't find tf2 directory")?
            .path
            .join("tf");
        let vpks = tf_dir
            .read_dir()?
            .filter_map(|item| item.ok())
            .filter_map(|item| Some(item.path().to_str()?.to_string()))
            .filter(|path| path.ends_with("dir.vpk"))
            .map(|path| vpk::from_path(&path))
            .filter_map(|res| res.ok())
            .collect();

        Ok(Loader { pack, tf_dir, vpks })
    }

    #[tracing::instrument]
    pub fn load(&self, name: &str) -> Result<Vec<u8>, Error> {
        debug!("loading {}", name);
        if let Some(data) = self.pack.get(name)? {
            debug!("got {} bytes from packfile", data.len());
            return Ok(data);
        }
        for vpk in self.vpks.iter() {
            if let Some(entry) = vpk.tree.get(name) {
                let data = entry.get()?.into_owned();
                debug!("got {} bytes from vpk", data.len());
                return Ok(data);
            }
        }
        error!("Failed to find {} in vpk", name);
        Err(Error::Other("Can't find file in vpks"))
    }
}
