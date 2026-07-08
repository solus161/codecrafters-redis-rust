[![progress-banner](https://backend.codecrafters.io/progress/redis/702d1917-1b9a-489d-9e27-beb85c5be2de)](https://app.codecrafters.io/users/codecrafters-bot?r=2qF)

This is a starting point for Rust solutions to the
["Build Your Own Redis" Challenge](https://codecrafters.io/challenges/redis).

In this challenge, you'll build a toy Redis clone that's capable of handling
basic commands like `PING`, `SET` and `GET`. Along the way we'll learn about
event loops, the Redis protocol and more.

**Note**: If you're viewing this repo on GitHub, head over to
[codecrafters.io](https://codecrafters.io) to try the challenge.

# Passing the first stage

The entry point for your Redis implementation is in `src/main.rs`. Study and
uncomment the relevant code, and push your changes to pass the first stage:

```sh
git commit -am "pass 1st stage" # any msg
git push origin master
```

That's all!

# Stage 2 & beyond

Note: This section is for stages 2 and beyond.

1. Ensure you have `cargo (1.94)` installed locally
1. Run `./your_program.sh` to run your Redis server, which is implemented in
   `src/main.rs`. This command compiles your Rust project, so it might be slow
   the first time you run it. Subsequent runs will be fast.
1. Commit your changes and run `git push origin master` to submit your solution
   to CodeCrafters. Test output will be streamed to your terminal.


# How To Use
You can have your own little Redis experience at your own terminal! Here's how:
- Open two terminals, better side-by-side. I use Terminator so a simple Ctrl+Shift+E split the whole thing into two.
- In one, run the server by `cargo run` or `./your_program.sh` which is an entry point. The default port is 6379. You can change this by introducing arg `--port 6379`.
- Run a script in `./src/tests/`, or use the official [Redis CLI](https://redis.io/docs/latest/develop/tools/cli/) to send command such as `redis-cli SET foo bar`.

Supported arguments:
- `--host`, default `127.0.0.1`
- `--port`, default `6379`
- `--replicaof`, set a upper stream `host port` as master

# What Are Implemented
As a learning project of one developer, this does not, and cannot, replicate all features of a proper Redis server, which is many-year effort of pros. However, I try my best to write everything from cratch to mimic core features, using Codecrafters' test harness. Here are the list of implemented features:
- Eventloop: based on epoll syscall, non-blocking. The underlying mechanism is more into system programming and it could be trivial for devs working with OS and low-level stuff. But not everyone has a chance to touch these, ehh?
- Timer mechanism for blocking commands: based on timer fd syscall, no polling whatsoever;
- RESP parser: supporting partial read, this is an interesting one;
- Basic commands `PING`, `SET`, `GET`;
- Data structures and operations: list, stream, transactions, blocking commands, optimistic locking, channels, sorted set, geospatial data;
- Replication;
- Snapshotting with RDB persistence, backed by AOF persistence; 

# Architecture
```
Event loop: registering for tcp listener, client fd, and timer fd
│
├──Loop 
   ├──Register new client
   ├──New stream comming in
   │  ├──RESP Parser
   │  ├──Command handler
   │     ├──In-memory store
   │     ├──Relication
   │     ├──Response to client
   │     ├──AOF persistence
   ├──Timer fd fired, a block command expired
   ├── Resolve satified blocking commands etc.


```

# Implementation Highlights
## Custom Event Loop
Based on [epoll](https://en.wikipedia.org/wiki/Epoll) syscall, the event loop is dead simple: a loop triggered when an I/O happens, processes sources in registering order. The benefit here is 1) no race condition: single thread, shared resources are accessed sequentially, 2) fast: an event triggered and corresponding codes run immediatly, no abtraction inbetween, no work scheduler or job stealing mechanism like Tokio; 3) lightweight: no overhead to serve multiple clients.

The implementation is in `./src/epoll.rs`. I borrowed most of it from this [blog post](https://www.zupzup.org/epoll-with-rust/), thx alot bro.

A good question before proceeding: "How does the kernel know when an I/O completed?", and that leads to interesting details.

### When A Signal Comes ..
When a device, in this case a network interface card (NIC), receives a stream of signal, it converts it to bytes, saves to its own buffer, then the following sequence runs:
1. The NIC does [DMA](https://en.wikipedia.org/wiki/Direct_memory_access) transfer the data to a preallocated region in RAM. This is done without the need of CPU, so the CPU does not need to stop what it is doing to do the copy. The allocated region consists of two parts: 1) fixed-sized a header/descriptor which is part of a ring structure - a [DMA ring](https://dev.to/ripan030/how-hardware-and-software-share-a-queue-understanding-dma-rings-pea); 2) a payload that the descriptor points to;
2. The NIC sends an [interrupt request](https://en.wikipedia.org/wiki/Interrupt_request) (IRQ) to the [Advanced Programmable Interrupt Controller](https://en.wikipedia.org/wiki/Advanced_Programmable_Interrupt_Controller). This one has one end (local APIC) integrated to each processor core, and the other end (I/O APIC) plugged into peripheral buses;
3. The APIC interrupts the core by raising the voltage on the cpu's end. The CPU checks its end at at the end of every instruction (this is a behavior);
4. If there is a surge of voltage, the CPU 1) stops what it is doing, saves current state to stack; 2) clear the [interrupt flag](https://en.wikipedia.org/wiki/Interrupt_flag) (IF) to prevent checking for interruption in following instructions; 3) match the IRQ with [Interrupt Descriptor Table](https://en.wikipedia.org/wiki/Interrupt_descriptor_table)) (IDT) to get a specific pointer that points to a handler, in this case, which is a piece of kerner code that does several other things;
5. The handler/kernel code acknowledges the interrupt, also set the IF flag again;
6. The kernel tranfers the data buffer to the `sk_buff`, the ring slot is free and ready to receive another package. This is not real "transfer" in the copy sense, but transfer of ownership of the payload. The kernel then checks the metadata of the package and map the `sk_buff`/package to the application's socket that about to read;
7. A callback `sk_data_ready()` is fired. The thread wakes up to continue what it was doing;
8. When the application call a `read()`, the data is copy from that kernel buffer to user space. This is when the real copy happens;

[More in to this...](https://kernel-internals.org/net/life-of-packet-rx/)

### More Onto The Kernel Buffer
"Kernel buffer" is a simplified concept. In reality, there is four components in actions:
- The DMA ring;
- [sk_buff](https://docs.kernel.org/networking/skbuff.html) associated with the data part of each ring slot. `sk_buff` could be created to own the data buffer so that the ring slot/descriptor could be freed. There could be more `sk_buff` than DMA ring slots.
- A [sock](https://docs.huihoo.com/doxygen/linux/kernel/3.7/structsock.html) struct which holds queue of available-to-read `sk_buff`, the `sk_receive_queue`. A ready-to-read `sk_buff`, after all medata is resolved (sender, receiver, host, port etc) is pointed to a `sock struct` and appended to this queue, while not-ready ones are not there. `sock` struct belongs to the the kernel space;
- A socket, this is the interface through which the application could interract with `sock` struct. A socket could hold many `sock` structs, each `sock` struct for a connection, as an application could have many clients/connections;

### Epoll, The Core of I/O Multiplexing
The above paragraph, we already reach the stage of a callback triggered when a package is ready. For that trigger, any many other triggers, to reach application in an organized way, we need epoll, an API that bridge the userspace and kernel space.

TL, DR: epoll watches fds for your, wakes the thread up for you, so the thread does not have to do polling that is costly.

#### Structure
The inner working of an epoll instance involve three components:
- The epoll instance, [eventpoll](https://github.com/torvalds/linux/blob/0e35b9b6ec0ffcc5e23cbdec09f5c622ad532b53/fs/eventpoll.c#L295) struct:
- The [epitem](https://github.com/torvalds/linux/blob/0e35b9b6ec0ffcc5e23cbdec09f5c622ad532b53/fs/eventpoll.c#L252) struct;
- The `epoll_event` which is part of `epitem`

```
┌────────────────────────────────────┐                                            
│ struct sock                        │                                            
│                                    │                                            
│ [sk_rx_skb_cache]───►sk_buff       │                                            
│ [sk_receive_queue]──►sk_buff_head  │                                            
│ [*sk_data_ready]                   │                                            
│             │                      │                                            
│             ├────►item: callback │ │                                            
│ [sk_wq]     ├────►item: callback │ │                                            
│             └────►..      │      ▼ │                                            
└───────────────────────────┼────────┘                                            
                    ┌───────┘                                                     
┌───────────────────▼────────────────┐  ┌────────────────┐                        
│          struct eventpoll          │┌─► struct epitem  │                        
│                                    ││ │                │                        
│  [rbr]]RB-Tree Root)               ││ │ [event]─────┐  │                        
│    │                               ││ └─────────────┼──┘                        
│    └─► [RB-Node] ──► [RB-Node]     ││               │                           
│                         │          ││               │                           
│                         ▼          ││ ┌─────────────▼─────────┐                 
│                     [*ovflist]─────┼┘ │ struct epoll_event    │                 
│                  ┌─────────────┐   │  │                       │                 
│  [rdllist]──────►│  rdllink    │   │  │ [events]──────────────┼──┬─►EPOLLIN     
│  (Doubly Linked) ├─────────────┤   │  │                       │  ├─►EPOLLOUT    
│                  │  ffd (fd)   │   │  │ [data]─►ffd(fd)       │  └─►EPOLLONESHOT
│                  └─────────────┘   │  └───────────────────────┘                 
└────────────────────────────────────┘                                            
```

When an event is registered with epoll instance (an event such as `EPOLLIN` for an fd):
- An `epitem` is created, having `events` bit mask and fd in data field;
- The `epitem` is added to the `eventpoll` RBR tree, this tree helps fast O(log n) lookup;
- An entry on the elevant sock (fd) is created and inserted to that sock wait queue `sk_wq`. This entry has  callback to the `epitem` and `eventpoll` above;
- When an package arrived, the associated `sk_buff` is created and insert into that sock `sk_receive_queue`, then `sk_data_ready` is called, which also calls callbacks of all item in `sk_wq`, which in turn call to `eventpoll`;
- The `eventpoll` callback runs and push the corresponding `epitem` to ready list `rdllist`;
- The thread/process could call `epoll_wait` on the fd of `eventpoll`: 1) if there are something ready, the fn pops up k `epitem` for k ready-to-read sources; 2) if there is nothing to consume, the thread is put to sleep on Wait queue till timeout/signal/new event;

### Triggering Behaviors: Edge vs Level
There is two triggering behaviors:
- Edge-triggered mode: when using mask `EPOLLIN` or `EPOLLOUT`. In this mode the epoll notifies the caller whenever there is data sitting in `sk_receive_queue`. So if the application does not consume packages, the epoll keeps notifying the application, which cause overhead;
- Level-triggered mode: when using the above masks with `EPOLLONESHOT`, the epoll will notify the caller once, then stops, even when there are still packages in queue. This is more efficient but the application has to consumes packages then rearm the epoll so future packages not notified. This is what we do in this toy project;

### Epoll is not async
However, epoll is a notification API, not real async, as the application still has to handle read/write by itself. For real async, there is a newer [io_using](https://en.wikipedia.org/wiki/Io_uring) which works for both disk and socket operations, while epoll only support network operations. However `io_using` is newer (2019) while epoll is much more stable and battle-tested (2002).

## RESP Parser Supporting Partial-Read
The RESP protocol is kinda straight-forward, so the parsing does not requires building abstract syntax tree (AST). However as a command is sent through TCP connection, there is a real risk of that command being broken into multiple pieces. As a result, tokenization based on delimiter and completed stream is out of the question. We have to resort to scanning the stream byte-by-byte.

An example of in partial read: a `*1\r\n\$4\r\nPING\r\n` could be sent as a series of:
```
*
1
\r
\n
$
4
\r
\n
P
I
N
G
\r
\n
```
That's an extreme edge case covered by `./src/tests/test1.sh`.

The implementation of the RESP parser is in `./src/resp.rs`.

In general, the RESP parser uses a mechanism of:
- 2 pointers to define the slice on the original buffer, this slice defines in-process token;
- A loop of through state machines where one defines the next, while keeping a target/token to be extracted. Each target token has two states: completed and incompleted;

Once a token is completed, its slice is extracted and pushed into a stack of completed tokens. Several completed tokens could be combined into a completed command structure, for example an array having several completed tokens/items. The completed command structure is passed to the command handler for execution.

The parser and command structure have several nice things:
- Zero copy: The copy of byte does not occured till a completed command structure is defined. As the buffer is later discard, the command needs to owns its own String, so the copy is still needed at the end;
- Survive delay: The parser does not need a completed command, and could survives great delay between bytes as long as all bytes will arrive at the end;
- Cheap to parse nested structure: The parser supported nested structure, for example array in array in array etc, without resorting to recursive;
- Nice API: Command strutures could be easily constructed and parsed into bytes, which help a lot with responding to clients.

## Blocking Command Not Blocking The Loop
Commands such as `BLPOP` and `XREAD BLOCK` could come with expirations. These kind of commands introduce some challenges:
- Commands could be fullfilled before expirations;
- Commands could be expired;
- When a new blocking command comes, a new expiration may overwrite the current expiration;
    
To solve these we need some extra data structure (btree for expiration) and epoll for controlling expiration.

### timerfd with epoll
Syscall `timerfd_create` basically creates a fd to timestamp, which could be registered with out epoll. When the timer on CPU reach that timestamp, the epoll fires, triggering our loop. However, we only need one timerfd for all blocking commands whether there are 1000, or 1 million blocking commands. All we need to do is reset/rearm the timerfd once an earlier expiration comes. For that we need a btree.

### btree with callbacks
The data structure we need for this kind of scheduler includes:
- A btree having expiration timestamp as key for each command and value holding expiration callback; as commands are processed sequentially, the timestamp is registered at the processed moment, not when package arrived, so no two commands having the same timestamp;
- A hashmap for storing metadata of blocking commands: what list/stream, what required count, what fullfillment callback;

The basic flow is like this:
- A blocking command with expiration comes, it is check for immediate fullfilment, immediate response to client if fullfilled;
- If not fullfilled: 1) registered with the hashmap, including fullfillment condtitions, fullfillment callback; 2) registered with the btree: expiration timestamp (key), and deadline/expiration callback;
- `timerfd` is rearmed with the earliest expiration timestamp;
- At the end of each round, list of unfullfilled commands is looped through. If a command is fullfilled, its callback fired, sending response to client. The command and is equivalent deadline/expiration is popped out of hashmap/btree, and `timerfd` is rearmed to ealiest timestamp on the tree;
- When the `timerfd` fired, the ealiest deadline is popped out of tree (and also from the hashmap), deadline callback fired, client got the expiration response;

# Known Rough Edges - Being Honest
- Silent errors: Several paths swallow `Result`s (AOF flush and broadcast are
  marked `// TODO: silent error here` in `main.rs`/`cmd_handler.rs`). Failures
  there won't surface to the client or logs yet.
- Single-threaded ceiling: One core, by design. Fine for learning and the test
  harness; not something to put real load on.
- `println!` logging: No structured logging or log levels — debugging output is
  commented-out `println!`s scattered through the hot paths.
- Partial command coverage: Core families are implemented, but many Redis
  commands, options, and edge cases (hash commands, `SCAN`, keyspace
  notifications, most `EXPIRE` variants, cluster mode) are absent.
- Linux only: It's built on `epoll`/`timerfd`, so it won't run on macOS or
  Windows without an equivalent (`kqueue`/IOCP) backend.
- Not hardened: No connection limits, no protocol fuzzing, no back-pressure on
  the write side beyond `write_all`. Treat it as a learning artifact, not
  production software.
