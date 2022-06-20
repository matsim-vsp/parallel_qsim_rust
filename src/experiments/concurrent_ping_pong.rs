/*
    Get started with concurrency. Try implementing something which has two threads that play ping
    pong and a main thread which waits for them to finish.
*/

use std::sync::mpsc;
use std::thread;

fn run() {
    let (tx1, rx1) = mpsc::channel();
    let (tx2, rx2) = mpsc::channel();

    let handle1 = thread::spawn(move || {
        let mut number_of_messages = 0;
        while number_of_messages < 10 {
            let ping = String::from("Ping");
            println!("Thread 1 sending: {ping:?}");
            tx1.send(ping).unwrap();

            let pong = rx2.recv().unwrap();
            println!("Thread 1 has received {}.", pong);
            number_of_messages += 1;
        }

        println!("Thread 1 is finishing")
    });

    let handle2 = thread::spawn(move || {
        let number_of_messages = 0;
        while number_of_messages < 9 {
            let ping = rx1.recv().unwrap();
            println!("Thread2 has received: {}", ping);
            println!("Thread2 sending: pong");
            tx2.send(String::from("pong")).unwrap();
        }

        println!("Thread 2 is finishing")
    });

    println!("Main thread has initialized 2 threads. Waiting on join now. ");
    handle1.join().unwrap();
    handle2.join().unwrap();
    print!("Both threads have finished. Program is Done.");
}

struct ChannelEnd {}

#[cfg(test)]
mod tests {
    use crate::experiments::concurrent_ping_pong::run;

    #[test]
    fn test_run() {
        run();
    }
}
