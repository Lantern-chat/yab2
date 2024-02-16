use std::path::Path;

use crate::*;

impl Client {
    pub async fn upload_from_path(
        &mut self,
        path: impl AsRef<Path>,
        mut info: NewFileInfo,
    ) -> Result<models::B2FileInfo, B2Error> {
        let path = path.as_ref();

        unimplemented!("WIP")
    }
}
