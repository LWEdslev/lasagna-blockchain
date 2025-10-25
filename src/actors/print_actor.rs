use actix::{Actor, Context, Handler};

use crate::actors::clock_actor::NewTimeslot;


// For testing other actors
pub struct PrintActor;

impl Actor for PrintActor {
    type Context = Context<Self>;
}

impl Handler<NewTimeslot> for PrintActor {
    type Result = ();

    fn handle(&mut self, msg: NewTimeslot, _: &mut Self::Context) -> Self::Result {
        println!("Got {:?} at {}", msg, crate::util::get_unix_timestamp());
    }
}