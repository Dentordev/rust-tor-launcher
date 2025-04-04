
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::fs::create_dir;
use std::io::{prelude::*, BufReader};
use std::net::TcpStream;
use std::process::{Command, Stdio, Child};

#[derive(Debug, Deserialize, Serialize)]
pub struct HiddenServiceConfig {
    port:u16,
    path:Option<String>,
    ssl_port:Option<u16>
}


impl HiddenServiceConfig {

    pub fn format_request(&self) -> String{
        let path = self.path.clone().unwrap_or(".hidden-service".into());
        let hs_path = Path::new(&path);
        if !hs_path.exists() {
            create_dir(hs_path).unwrap();
        }

        // On windows, tor has a hard time parsing paths so we need to convert on over to unix...
        let hs_dir = path.replace("\\", "/");
        
        format!("SETCONF HiddenServiceDir=\"{}\" HiddenServicePort=\"80 127.0.0.1:{}\"\r\n", hs_dir, self.port) 
    }
 
}


#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    config_port:u16,
    socks_port:Option<u16>,
    /// Yaml string for the command we wish to execute after tor is launched
    command:String,
    tor_exe:Option<String>,
    tor_password:Option<String>,
    hidden_services:Vec<HiddenServiceConfig>
}



/// Forked from torut 
pub fn run_tor<A, T, P>(path: P, args: A) -> Result<Child, std::io::Error>
    where
        A: AsRef<[T]>,
        T: AsRef<str>,
        P: AsRef<str>,
{
    let path = path.as_ref();
    let mut c = Command::new(path)
        .args(args.as_ref().iter().map(|t| t.as_ref()))
        // .env_clear()
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::piped())
        .spawn()?;
    {
        // Stdio is piped so this works
        {
            let mut stdout = BufReader::new(c.stdout.as_mut().unwrap());

            loop {
                // wait until tor starts
                // hacky but works
                // stem does something simmilar internally but they use regexes to match bootstrapping messages from tor
                //
                // https://stem.torproject.org/_modules/stem/process.html#launch_tor

                let mut l = String::new();
                match stdout.read_line(&mut l) {
                    Ok(v) => v,
                    Err(e) => {
                        // kill if tor process hasn't died already
                        // this should make sure that tor process is not alive *almost* always
                        let _ = c.kill(); 
                        return Err(e);
                    }
                };

                if l.contains("Opened Control listener") {
                    break;
                }
            }
            
            // buffered stdout is dropped here.
            // It may cause partial data loss but it's better than dropping child.
        }
    }
    Ok(c)
}


impl Config {
    /// Launch tor process along with our command soon after
    pub fn run(&self){
        let mut vec_config:Vec<String> = Vec::new();
        vec_config.push("ControlPort".into());
        vec_config.push(format!("{}", self.config_port).into());
        
        // We need to be able to control shutdown, you can find this exact string in the stem python library.
        vec_config.push("__OwningControllerProcess".into());
        vec_config.push(format!("{}", std::process::id()).into());


        if self.socks_port.is_some(){
            // hmap.insert("SocksPort".into(), format!("{}", self.socks_port.unwrap()));
            vec_config.push("SocksPort".into());
            vec_config.push(format!("{}", self.socks_port.unwrap()).into());
        }
        
        println!("Launching Tor");

        let mut child = run_tor("tor", vec_config).unwrap();

        // let mut child = launch_tor_with_config(self.tor_exe.clone(), hmap, true).unwrap();
        println!("Connecting to Config And Authenticating");
        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", self.config_port)).unwrap();
        
        if self.tor_password.is_none() {
            stream.write(b"AUTHENTICATE\r\n").unwrap();
        } else {
            stream.write(format!("AUTHENTICATE \"{}\"", self.tor_password.clone().unwrap()).as_bytes()).unwrap();
        }
        stream.flush().unwrap();
        
        let mut data = [0; 8];
        stream.read( &mut data[..]).unwrap();
        assert!(&data == b"250 OK\r\n", "AUTHENTICATION FAILED!");
        
        println!("Setting Up Hidden Services");
        for hidden_service in &self.hidden_services {
            let line = hidden_service.format_request();
            println!("{}", line);
            stream.write(line.as_bytes()).unwrap();
            stream.flush().unwrap();
            stream.read( &mut data[..]).unwrap();
            assert!(&data == b"250 OK\r\n", "HIDDEN SERVICE LAUNCH FAILED!");
        }
        println!("Launching Final Command...");

        let mut command = if cfg!(target_os="windows") {
            Command::new("cmd")
                .arg("/C")
                .arg(&self.command)
                .stdout(Stdio::piped())
                .spawn().unwrap()
                
        } else {
            Command::new("sh")
                .arg("-c")
                .args(self.command.split(" "))
                .stdout(Stdio::piped())
                .spawn().unwrap()
        };
        let mut reader = BufReader::new(command.stdout.as_mut().unwrap());
        let mut line = String::new();
        println!("Waiting for data...");
        while reader.read_line(&mut line).unwrap() > 0 {   
            print!("{}", line);
            line.clear();
        }

        command.wait().unwrap();
        println!("Shutting down...");
        child.kill().unwrap();


    }
}


