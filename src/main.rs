pub mod tor_controller;
use tor_controller::Config;
use std::fs::{File, exists};
use std::io::Write;
use std::path::Path;
use serde_yml;


static DEFAULT_CONFIG: &[u8] = b"
config_port: 9051
socks_port: 9050
tor_exe: tor

# Leave blank if you don't have a tor password
tor_password:

# Please fill these parts out 
# if for whatever reason the Command Does Not Run, 
# use a batch or shell script...
# if you need sudo permissions, run sudo tor-launcher before running
command:

hidden_services:
  # Path Default to current directory under .hidden-service
  # You can add more hidden services just know that they won't run the same with 
  # as multiple commands at one time hasn't been implemented...

  - port: 6667
    path:
    ssl_port:
";

fn main() {
    let cfg_path= Path::new("config.yaml");
    if !exists(cfg_path).unwrap(){
        println!("Yaml Config doesn't exist so we will generate one for you...");
        let mut new_file = File::create_new("config.yaml").unwrap();
        new_file.write(DEFAULT_CONFIG).unwrap();
        println!("Run this script again when you've filled out your config file...")
    } else {
        let file = File::open("config.yaml").unwrap(); 
        
        let config:Config = serde_yml::from_reader(file).unwrap();
        println!("Preparing to run...");
        config.run();


    }
 
}
