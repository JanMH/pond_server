use std::{
    panic, process::{Command, ExitStatus, Stdio}, thread
};

use crate::deployer::DeploymentHandle;

pub fn run_command(
    mut command: Command,
    mut message_stream: DeploymentHandle,
) -> std::io::Result<ExitStatus> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut spawned = command.spawn()?;
    let mut stdout = spawned.stdout.take().unwrap();
    let mut stderr = spawned.stderr.take().unwrap();

    let mut cloned = message_stream.clone();
    let err_jh = thread::spawn(move || {
        std::io::copy(&mut stderr, cloned.error())?;
        debug!("stderr copied");
        Ok::<(), std::io::Error>(())
    });
    std::io::copy(&mut stdout, message_stream.info())?;

    let result = spawned
        .wait()
        .inspect(|_r| debug!("Command terminated successfully: {:?}", command))
        .inspect_err(|e| error!("Command {:?} failed {:?}",command, e));
    
    match err_jh.join() {
        Ok(result) => {
            result?;
        }
        Err(e) => panic::resume_unwind(e),
    }
    result
}

#[cfg(test)]
mod test {
    use super::*;
    use std::{io, process::Command};

    use crate::deployer::handle::deployment_handle;

    #[test]
    fn test_run_command() {
        let mut command = Command::new("echo");
        command.arg("Hello!");
        let (write, mut read) = deployment_handle();
        run_command(command, write).expect("Could not launch echo command");

        let output = io::read_to_string(read.info()).expect("Could not read command output");
        assert_eq!(output, "Hello!\n")
    }
}
