use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::fs::OpenOptions;

use crate::app_state::Configs;
use crate::{BUFFER_SIZE};
use crate::cmd_builder::Cmd;
use crate::cmd_handler::{CmdHandler};
use crate::exceptions::CustomError;
use crate::resp::RespParser;

pub struct Aof {
    manifest_filepath: Option<String>,
    aof_filepath: Option<String>,
    manifest_file: Option<fs::File>,
    aof_bufwriter: Option<BufWriter<fs::File>>,
}

impl Aof {
    pub fn new() -> Self {
        Self {
            manifest_filepath: None,
            aof_filepath: None, 
            manifest_file: None,
            aof_bufwriter: None,
        }
    }

    pub fn init_sequence(&mut self, parser: RespParser, cmd_handler: &mut CmdHandler) -> Result<(), CustomError> {
        let configs = Configs::get();
        if !configs.appendonly() { return Ok(())};

        let append_dirpath = format!(
            "{}/{}",
            configs.path().as_deref().expect("No base path"),
            configs.appenddirname());

        let dir_exists = std::path::Path::new(&append_dirpath).exists();

        if dir_exists {
            let _ = self.aof_replay(parser, cmd_handler)?;
            self.init_aof_files()?;
        } else {
            self.create_dirs()?;
            self.init_aof_files()?;
        };
        Ok(())
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

    pub fn init_aof_files(&mut self) -> Result<(), CustomError> {
        // If the appendfilename = "appendonly.aof"
        // subsequent created files will have name appendonly.aof.x.incr.aof_dir
        // with x increasing integer
        // If append only directory exists, read manifest
        // else creaste append only directory and 
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
        let aof_file = OpenOptions::new()
            .create(true).append(true).open(&aof_filepath)
            .expect(&format!("Error opening AOF file {}", &aof_filepath));

        // Got hold of the aof file
        self.aof_filepath = Some(aof_filepath);
        self.aof_bufwriter = Some(BufWriter::new(aof_file));

        // Manifest file
        let manifest_filepath = format!(
            "{}/{}.manifest",
            &append_dirpath,
            &configs.appendfilename());
        self.manifest_filepath = Some(manifest_filepath.clone());

        let mut manifest_file = fs::File::create(&manifest_filepath).expect(
            &format!("Error creating manifest file {}", &manifest_filepath));
        let first_line = format!("file {}.{}.incr.aof seq {} type i\n",
            &configs.appendfilename(), id, id);
        manifest_file.write_all(first_line.as_bytes())
            .expect("Error writing manifest file");

        Ok(())
    }

    pub fn aof_write(&mut self, line: &[u8]) -> Result<(), CustomError> {
        let Some(buf) = &mut self.aof_bufwriter else {
            return Err(CustomError::InternalError("Error writing AOF file".to_string()));
        };

        let _ = buf.write(line)?;
        Ok(())
    }

    pub fn aof_flush(&mut self) -> Result<(), CustomError> {
        let Some(buf) = &mut self.aof_bufwriter else {
            return Err(CustomError::InternalError("Error writing AOF file".to_string()));
        };

        let _ = buf.flush()?;
        Ok(())
    }

    pub fn aof_replay(&mut self, mut parser: RespParser, cmd_handler: &mut CmdHandler)
        -> Result<(), CustomError>
    {
        // Read the manifest file to get the filename
        // Append-only dir must exist
        let configs = Configs::get();
        let manifest_filepath = format!(
            "{}/{}/{}.manifest",
            configs.path().expect("No path set"),
            configs.appenddirname(),
            configs.appendfilename(),
        );

        let file = fs::File::open(manifest_filepath)?;
        let buf_reader_manifest = BufReader::new(file);
        
        let mut aof_filename: String = String::new();
        let mut aof_seq: u64 = 1;
        let mut aof_type: String = String::new();

        for line in buf_reader_manifest.lines() {
            let line = line?;
            let splitted: Vec<&str> = line.split(' ').collect();

            let mut i = 0;
            while i < splitted.len() {
                match splitted[i] {
                    "file" => {
                        aof_filename = splitted[i+1].to_string()
                    },
                    "seq" => {
                        aof_seq = splitted[i+1].to_string().parse::<u64>()?
                    },
                    "type" => {
                        aof_type = splitted[i+1].to_string()
                    },
                    _ => {/* not implemented */}
                }
                i += 1;
            };

            if aof_type == "i" { break }
        };

        if aof_type != "i" {
            return Err(CustomError::InternalError("No aof type i found".to_string()))
        };

        
        let aof_filepath = format!(
            "{}/{}/{}",
            configs.path().expect("No folder path"),
            configs.appenddirname(),
            &aof_filename
        );
        
        let aof_file = fs::File::open(aof_filepath)?;
        let mut buf_reader = BufReader::new(aof_file);
        let mut buf = [0u8; BUFFER_SIZE as usize];

        // Start replay
        loop {
            let n = match buf_reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) => return Err(e.into())
            };

            parser.feed_buf(&buf, n);

            loop {
                parser.parse(false);
                match parser.get_completed() {
                    Some(resp_type) => {
                        let raw = resp_type.serialize();
                        let cmd = Cmd::from_resp(resp_type);

                        cmd_handler.handle(cmd, raw, 0, -1);
                    },
                    None => break,
                }
            }
        };
        Ok(())
    }
}
