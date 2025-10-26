use lasagna_blockchain::{actors::{clock_actor::{ClockActor, Subscribe}, print_actor::{self}}, util::START_TIME};
use actix::Actor;

#[actix::main]
async fn main() {
    let clock_actor = ClockActor::new().start();
    tokio::spawn(ClockActor::run_loop(clock_actor.clone(), START_TIME));

    let print_actor = print_actor::PrintActor.start();

    clock_actor.do_send(Subscribe(print_actor.recipient()));

    tokio::signal::ctrl_c().await.unwrap();
}






