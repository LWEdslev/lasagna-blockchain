use std::{collections::HashSet, time::Duration};

use actix::{Actor, Addr, Context, Handler, Message, Recipient};

use crate::util::{Timeslot, SLOT_LENGTH};


/// Notifies subscribers when a new timeslot is reached
pub struct ClockActor {
    subscribers: HashSet<Recipient<NewTimeslot>>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Subscribe(pub Recipient<NewTimeslot>);

impl ClockActor {
    pub fn new() -> Self {
        Self {
            subscribers: Default::default(),
        }
    }

    pub async fn run_loop(addr: Addr<Self>, start_time: u128) {
        let mut curr_timeslot = crate::util::calculate_timeslot(start_time);
        addr.do_send(NewTimeslot(curr_timeslot));
        loop {
            let next_timeslot = curr_timeslot + 1;
            let next_timeslot_start = start_time + SLOT_LENGTH * (next_timeslot as u128);
            let time_to_sleep = next_timeslot_start.saturating_sub(crate::util::get_unix_timestamp());
            tokio::time::sleep(Duration::from_micros(time_to_sleep as _)).await;
            let new_timeslot = crate::util::calculate_timeslot(start_time);
            if new_timeslot != curr_timeslot {
                curr_timeslot = new_timeslot;
                addr.do_send(NewTimeslot(new_timeslot));
            }
        }
    }
}

impl Handler<NewTimeslot> for ClockActor {
    type Result = ();

    fn handle(&mut self, msg: NewTimeslot, _: &mut Self::Context) {
        self.subscribers.iter().for_each(|sub| sub.do_send(msg));
    }
}

impl Handler<Subscribe> for ClockActor {
    type Result = ();
    
    fn handle(&mut self, msg: Subscribe, _: &mut Self::Context) -> Self::Result {
        self.subscribers.insert(msg.0);
    }
}

impl Actor for ClockActor {
    type Context = Context<Self>;
}

#[derive(Message, Clone, Copy, Debug)]
#[rtype(result = "()")]
pub struct NewTimeslot(pub Timeslot);

