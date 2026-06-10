use std::fs;
use std::io::{Write, BufWriter};

use crate::app_state::Configs;
use crate::exceptions::CustomError;

pub struct Aof {
    manifest_filepath: Option<String>,
    aof_filepath: Option<String>,
    manifest_file: Option<fs::File>,
    aof_file: Option<BufWriter<fs::File>>,
}

impl Aof {
    pub fn new() -> Self {
        Self {
            manifest_filepath: None,
            aof_filepath: None, 
            manifest_file: None,
            aof_file: None,
        }
    }

    pub fn create_dirs(&self) -> Result<(), CustomError> {
        let configs = Configs::get();
        // The appendonly flag trigger the creation
        if !configs.appendonly() { return Ok(())};

        if let Some(path) = configs.path() {
            fs::create_dir_all(path).expect("Error create base dir"); 
        };

        if !configs.appenddirname().is_empty() {
            let aof_dir = match &configs.path() {
                Some(base) => format!("{}/{}", base, configs.appenddirname()),
                None => configs.appenddirname().to_string(),
            };
            fs::create_dir_all(aof_dir).expect("Error create AOF dir");
        };

        Ok(())
    }

    fn get_latest_aof_id(&self) -> Option<u64> {
        let configs = Configs::get();
        let aof_dir = format!(
            "{}/{}",
            &configs.path().as_deref().expect("No base path"),
            &configs.appenddirname()
            );

        let msg_err = "Error reading append dir";
        let id = fs::read_dir(aof_dir).expect(msg_err)
            .filter_map(|e| {
                let entry = e.expect(msg_err);
                let name = entry.file_name().into_string()
                    .expect(msg_err);

                let splitted_name: Vec<&str> = name.split('.').collect();
                splitted_name[2].parse::<u64>().ok()
            }).max();
        id
    }

    pub fn create_aof_files(&mut self) -> Result<(), CustomError> {
        // If the appendfilename = "appendonly.aof"
        // subsequent created files will have name appendonly.aof.x.incr.aof_dir
        // with x increasing integer
        let configs = Configs::get();
        if !configs.appendonly() { return Ok(())}

        let append_dirpath = format!(
            "{}/{}",
            &configs.path().as_deref().expect("No base path"),
            &configs.appenddirname());
        
        // AOF file
        let id = self.get_latest_aof_id().unwrap_or(1);
        let aof_filepath = format!(
            "{}/{}.{}.incr.aof",
            &append_dirpath,
            &configs.appendfilename(),
            &id);
        let aof_file = fs::File::create(&aof_filepath).expect(
            &format!("Error creating AOF file {}", &aof_filepath));

        // Got hold of the aof file
        self.aof_filepath = Some(aof_filepath);
        self.aof_file = Some(BufWriter::new(aof_file));

        // Manifest file
        let manifest_filepath = format!(
            "{}/{}.manifest",
            &append_dirpath,
            &configs.appendfilename());
        let mut manifest_file = fs::File::create(&manifest_filepath).expect(
            &format!("Error creating manitest file {}", &manifest_filepath));
        self.manifest_filepath = Some(manifest_filepath);
        
        // This write could be better
        let first_line = format!("file {}.{}.incr.aof seq {} type i\n",
            &configs.appendfilename(), id, id );
        let _ = manifest_file.write_all(&first_line.as_bytes());

        Ok(())
    }

    pub fn aof_write(&mut self, line: &[u8]) -> Result<(), CustomError> {
        let Some(buf) = &mut self.aof_file else {
            return Err(CustomError::InternalError("Error writing AOF file".to_string()));
        };

        let _ = buf.write(line)?;
        Ok(())
    }

    pub fn aof_flush(&mut self) -> Result<(), CustomError> {
        let Some(buf) = &mut self.aof_file else {
            return Err(CustomError::InternalError("Error writing AOF file".to_string()));
        };

        let _ = buf.flush()?;
        Ok(())
    }
}
