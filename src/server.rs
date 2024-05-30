use std::time::{ Duration, Instant };
use redis::Commands;
use actix::prelude::*;
use actix_web_actors::ws;
use serde_json;
use serde::{ Serialize, Deserialize };

/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

/// websocket connection is long running connection, it easier
/// to handle with an actor
pub struct MyWebSocket {
    /// Client must send ping at least once per 10 seconds (CLIENT_TIMEOUT),
    /// otherwise we drop connection.
    hb: Instant,
    redis_con: Option<redis::Connection>,
}

#[derive(Serialize, Deserialize)]
struct AnimationMetadata {
    name: String,
    repeat: i32,
    text: Option<String>,
}

impl MyWebSocket {
    pub fn new() -> Self {
        Self { hb: Instant::now(), redis_con: None }
    }

    /// helper method that sends ping to client every 5 seconds (HEARTBEAT_INTERVAL).
    ///
    /// also this method checks heartbeats from client
    fn hb(&self, ctx: &mut <Self as Actor>::Context) {
        //  the client should send ping/pong message within the  CLIENT_TIMEOUT period,
        // because the heatbeat is checking, if it expires, the server will close the context.
        // each time client send a ping (maybe also pong) message, it reset the expire time.
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // check client heartbeats
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                // heartbeat timed out
                println!("Websocket Client heartbeat failed, disconnecting!");

                // stop actor
                ctx.stop();

                // don't try to send a ping
                return;
            }

            ctx.ping(b"");
        });
    }
}

///
// One connection per actor:

// Pros:

// Isolation: Each actor has its own dedicated connection, ensuring isolation and avoiding potential race conditions or interference.
// Simplicity: Easier to manage and debug as connections are not shared.
// Scalability: Can handle a higher number of concurrent actors as each has its own connection.

// Cons:

// Resource usage: Creates more connections, leading to higher memory and CPU consumption on the Redis server and your application.
// Connection overhead: Establishing and maintaining multiple connections can add overhead, impacting performance.
// Single connection for all actors:

// Pros:

// Resource efficiency: Uses only one connection, reducing memory and CPU overhead.
// Lower connection overhead: Less time spent establishing and maintaining connections.

// Cons:

// Complexity: Sharing a connection requires careful synchronization and error handling to avoid conflicts and race conditions.
// Scalability limitations: Might reach the maximum allowed connections on the Redis server if the number of actors grows significantly.
// Performance bottlenecks: Shared connections could become bottlenecks during high concurrency, impacting performance for all actors.

// Redis nodes can have up to either 10,000 simultaneous connections
// or 4 simultaneous connections per megabyte of memory, whichever is larger.
///

impl Actor for MyWebSocket {
    type Context = ws::WebsocketContext<Self>;

    /// Method is called on actor start. We start the heartbeat process here.
    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);

        // Establish Redis connection here
        let client = redis::Client
            ::open("redis://localhost:6379")
            .expect("Failed to connect to Redis");
        let con = client.get_connection().expect("Failed to get Redis connection");

        // Store the connection in a field for later use
        self.redis_con = Some(con);
    }
}

/// Handler for `ws::Message`
/// Handler for `ws::Message`
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MyWebSocket {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        // process websocket messages
        println!("WS: {msg:?}");
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                // Access the Redis connection
                if let Some(con) = &mut self.redis_con {
                    // Access and use each part here
                    if text.contains(":") {
                        let delimiter_index = text.find(":").expect("Failed to find delimiter");

                        let prefix = &text[..delimiter_index + 1]; // includes delimiters
                        // let suffix = &text[delimiter_index + 1..];

                        // covert text from bytestring::ByteString to &str
                        let reids_key = text.as_ref();

                        match prefix {
                            "am:" => {
                                let value: String = con
                                    .get::<&str, String>(&reids_key)
                                    .expect("Failed to read data from Redis");

                                println!(
                                    "fetched data from redis, size {}",
                                    value.as_bytes().len()
                                );

                                // Concatenation here:
                                let message = format!("{}::{}", reids_key, value);

                                let msg_len = message.as_bytes().len();

                                ctx.text(message); // Send the concatenated message

                                println!("send messahe to client, size {}", msg_len);
                            }
                            "amq:" => {
                                let value: Vec<String> = con
                                    .lrange(&reids_key, 0, -1)
                                    .expect("Failed to read list from Redis");

                                // iterate over the list, convert string item to json object
                                let values: Vec<AnimationMetadata> = value
                                    .iter()
                                    .map(|json_string|
                                        serde_json
                                            ::from_str(json_string)
                                            .expect("Failed to parse json string")
                                    )
                                    .collect();

                                println!("list size {}", values.len());

                                // Concatenation here:
                                let message = format!(
                                    "{}::{}",
                                    reids_key,
                                    serde_json
                                        ::to_string(&values)
                                        .expect("Failed to serialize list to string")
                                );

                                let msg_len = message.as_bytes().len();

                                ctx.text(message); // Send the concatenated message

                                // todo iterate over list, and get each animation data from redis
                                // but do this on the client side, to reduce the size of data sent over the network

                                println!("send messahe to client, size {}", msg_len);
                            }
                            _ => {
                                println!("received unknown text {}", text);
                            }
                        }
                    } else {
                        println!("received unknown text {}", text);
                    }
                } else {
                    // Handle the case where the connection is not established
                    println!("Redis connection not available");
                }
            }
            Ok(ws::Message::Binary(bin)) => ctx.binary(bin),
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => ctx.stop(),
        }
    }
}
