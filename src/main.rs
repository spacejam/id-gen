use rand::{seq::SliceRandom, thread_rng, Rng};
use uuid::Uuid;

const N_SERVERS: usize = 10;
const N_CLIENTS: usize = 15;

type Id = u64;
type Success = bool;
type To = usize;
type From = usize;

#[derive(Debug, Clone)]
enum Message {
    // request ID and proposed ID
    Request {
        uuid: Uuid,
        id: Id,
    },

    // proposal accepted?, request ID, server's highest known ID
    Response {
        success: Success,
        uuid: Uuid,
        id: Id,
    },
}

#[derive(Debug)]
enum Computer {
    Server(Server),
    Client(Client),
}

impl Computer {
    fn receive(&mut self, from: From, message: Message) -> Vec<(To, Message)> {
        match (self, message) {
            (Computer::Server(server), Message::Request { uuid, id }) => {
                server.propose(from, uuid, id)
            }
            (Computer::Client(client), Message::Response { success, uuid, id }) => {
                client.receive(from, success, uuid, id)
            }
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Default)]
struct Server {
    max_id: u64,
}

impl Server {
    fn propose(&mut self, from: From, uuid: Uuid, id: Id) -> Vec<(To, Message)> {
        if id > self.max_id {
            self.max_id = id;
            return vec![(from, Message::Response { success: true, uuid, id })];
        }
        vec![(from, Message::Response { success: false, uuid, id: self.max_id })]
    }
}

#[derive(Debug, Default)]
struct Client {
    last_id: Id,

    // in-flight request ID
    current_uuid: Uuid,
    current_responses: Vec<Result<Id, Id>>,
}

impl Client {
    fn generate_requests(&mut self) -> Vec<(To, Message)> {
        let mut ret = vec![];

        let new_uuid = Uuid::new_v4();
        self.current_uuid = new_uuid;
        self.current_responses.clear();

        for id in 0..N_SERVERS {
            ret.push((
                id,
                Message::Request {
                    uuid: new_uuid,
                    id: self.last_id + 1,
                },
            ))
        }

        ret
    }

    fn receive(&mut self, from: From, success: Success, uuid: Uuid, id: Id) -> Vec<(To, Message)> {
        if uuid != self.current_uuid {
            return vec![];
        }

        if success {
            assert_eq!(id, self.last_id + 1);
            self.current_responses.push(Ok(id));

            if self.current_responses.iter().filter(|r| r.is_ok()).count() > N_SERVERS / 2 {
                assert!(self.last_id < id);
                self.last_id = id;
                self.current_uuid = Uuid::new_v4();
                println!("SUCCESS; ID = {}", id);
            }
        } else {
            self.current_responses.push(Err(id));

            if self.current_responses.iter().filter(|r| r.is_err()).count() > N_SERVERS / 2 {
                self.last_id = id;
                println!("FAILURE; ID = {}", id);
                return self.generate_requests();
            }
        }

        vec![]
    }
}

fn main() {
    // fake cluster
    let mut in_flight: Vec<(From, To, Message)> = vec![];
    let mut computers = vec![];

    for _ in 0..N_SERVERS {
        computers.push(Computer::Server(Server::default()));
    }
    for _ in 0..N_CLIENTS {
        computers.push(Computer::Client(Client::default()));
    }

    // seed initial requests
    for sender in N_SERVERS..N_SERVERS + N_CLIENTS {
        let client = if let Computer::Client(client) = &mut computers[sender] {
            client
        } else {
            unreachable!()
        };

        let outbound = client.generate_requests();

        for (to, message) in outbound {
            in_flight.push((sender, to, message));
        }
    }

    loop {
        if in_flight.is_empty() {
            return;
        }

        let (from, to, message) = in_flight.pop().unwrap();

        // println!("from={} to={} message={:?}", from, to, message);
        let outbound = computers[to].receive(from, message);

        let mut rng = thread_rng();
        for (destination, message) in outbound {
            if rng.gen_ratio(1, 10) {
                // just drop the outbound message
                // simulates loss
                // XXX continue;
            }
            in_flight.push((to, destination, message));
        }

        // chaos
        in_flight.shuffle(&mut rng);
    }
}
