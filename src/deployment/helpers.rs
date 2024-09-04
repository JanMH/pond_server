use std::{
    panic,
    process::{Command, ExitStatus, Stdio},
    thread,
};

use crate::message::MessageSender;

pub fn copy_command_results(
    mut command: Command,
    mut message_stream: MessageSender,
) -> std::io::Result<ExitStatus> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut spawned = command.spawn()?;
    let mut stdout = spawned.stdout.take().unwrap();
    let mut stderr = spawned.stderr.take().unwrap();

    let mut cloned = message_stream.clone();
    let jh = thread::spawn(move || std::io::copy(&mut stderr, cloned.error()));
    std::io::copy(&mut stdout, message_stream.info())?;
    match jh.join() {
        Ok(result) => {
            result?;
        }
        Err(e) => panic::resume_unwind(e),
    }

    spawned.wait()
}

#[cfg(test)]
mod test {
    use super::*;
    use std::{io, process::Command};

    use crate::message::message_channel;

    #[test]
    fn test_run_command() {
        let mut command = Command::new("echo");
        command.arg("Hello!");
        let (write, mut read) = message_channel();
        copy_command_results(command, write).expect("Could not launch echo command");

        let output = io::read_to_string(read.info()).expect("Could not read command output");
        assert_eq!(output, "Hello!\n")
    }
}
