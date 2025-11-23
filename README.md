# README

Author: `Isaac Cespedes`

## Problem Description

Backend Engineering Challenge
Build a real-time system for fulfillment of food orders in a delivery-only kitchen. Your system
should allow orders to be received, stored and eventually picked up. All of these events can
happen concurrently.
Order fulfillment
Food orders have the following attributes:
● ID. A short identifier (e.g., e6bc2) used to identify the correct order at pick up.
● Name. The name of food (e.g., Cheese Pizza).
● Temperature. Ideal storage temperature: hot, cold or room.
● Price. Order price in dollars.
● Freshness. Duration (in seconds) the food remains fresh at its ideal temperature.
An order not stored at its ideal temperature degrades twice as quickly.
Placing orders
When a new order is received, the system instantly cooks the order and stores it until pick up.
The following options are available for storage:
● Cooler. Holds up to 6 cold orders at cold temperature.
● Heater. Holds up to 6 hot orders at hot temperature.
● Shelf. Holds up to 12 orders at room temperature.
To ensure the food remains as fresh as possible, the system should try to store a cooked order
at its ideal temperature. If the ideal option is full, the order must be placed on the shelf. If the
shelf is also full, the system should first move an existing, cold or hot order on the shelf to either
the cooler or heater, respectively, if there is room. If no such move is possible, an order stored
on the shelf must be selected – thoughtfully using a criteria of your choice – and discarded first
to make room for the new order.
Removing orders
When an order is picked up, it must be removed quickly without moving other orders around. No
action is taken by the kitchen if the order is not present.
If an order has exceeded its freshness duration at the time of pickup, it must be discarded
instead.

Capturing kitchen actions
The system should capture the sequence of kitchen actions along with at least timestamp, order
id and target (heater, cooler or shelf). The actions are:
● place: a new order is stored per the placement logic after making room, if needed
● move: an order is moved from the shelf to the cooler or heater as appropriate
● pickup: an order is picked up
● discard: an order is discarded
The sequence of actions forms a ledger of how orders are managed. The first action for any
order is always place and the last is either pickup or discard. The actions should be output
to console in a human-readable form with adequate information to follow along in real time.
Execution harness
The solution must include a simple harness to exercise the real-time system, bundled together
as a single-process command line program. The program must exercise order placement and
pickup based on the following configuration parameters:
● Rate: the rate at which orders should be placed (default: 500ms between orders)
● Pickup: the closed time interval in which pickup should occur (default: 4-8 seconds)
The harness must fetch a list of orders from our challenge server and place them one by one at
the given rate for test purposes. Each order must be picked up a random amount of time within
the pickup interval after it has been placed. After all orders have been fully processed, the
program must submit the captured list of actions, output the result and exit.
The challenge server generates random test problems and helps you validate your solution. You
may try as many test problems as you want. It requires a personal authentication token,
which is provided separately.

When we evaluate the challenge, we look at how focused you were on meeting
the requirements, at the simplicity and correctness of your architecture, your choices of
concurrency primitives and data structures as well as your discipline and command of the
language you chose.

Requirements
Please submit your solution in a zip file with the following:
● Code: Correct, production-quality code implementing order fulfillment as described along
with a Dockerized command-line runnable execution harness. The submitted code must
validate solutions against the challenge server and consistently pass. Please include
comments where appropriate as you would in a real project, but no need to document
every public class/method.
● Concurrency: Your order-fulfillment system should assume that orders may be placed
and removed concurrently and that it may be used outside the limited execution harness.
Your solution must run as a single process, possibly multi-threaded.
● Efficiency: The discard logic should have better than linear worst-case time complexity
in terms of the size of the shelf.
● README: A brief README file that explains how to build and run the harness program
as well as a paragraph or two on how you select an order to discard when the shelf is full
and why you chose that criteria. If aspects of the challenge are ambiguous or unclear to
you, justify your understanding and decisions in the README.

Appendix
Challenge server
The Challenge server exposes the following operations. It uses UTF8 encoding and standard
HTTP status codes. If you use the scaffold, you don’t need to interact with the server directly.
New problem
Generates a seeded-random test problem. Provide the same seed to get the same response.

Request:
GET https://<CHALLENGE_ENDPOINT>/challenge/new
Query params:
Name Type Description
auth string Authentication token (required)
name string Leave empty (optional)
seed int64 Problem seed (optional)
Response:
Headers:
Name Type Description
x-test-id string Test id
Content-Type string application/json
The body contains a list of orders encoded as json as follows:
[
{
"id": "e6bc2", // Order id
"name": "Cheese Pizza", // Order name
"temp": "hot", // Ideal storage temperature
"price": 10, // Price in dollars
"freshness": 120 // Freshness duration (in seconds)
},
...
]

Solve problem
Submits a solution to a test problem.
Request:
POST https://<CHALLENGE_ENDPOINT>/challenge/solve

Query params:
Name Type Description
auth string Authentication token (required)
Headers:
Name Type Description
x-test-id string Test id (required)
Content-Type string application/json
The body must contain a list of actions and parameters encoded as json as follows:
{
"options": {
"rate": 500000, // Inverse order rate (in microseconds)
"min": 4000000, // Min pickup time (in microseconds)
"max": 8000000 // Max pickup time (in microseconds)
},
"actions": [{
"timestamp": 1667667202596000, // Unix timestamp (in microseconds)
"id": "e6bc2", // Order id
"action": "place" // Action: place, move, pickup or discard
"target": "cooler" // Target: heater, cooler or shelf
},
...
]
}
Response:
The body contains a string with the pass/fail test result.

## How to run

The `Dockerfile` defines a self-contained Rust reference environment.
Build and run the program using [Docker](https://docs.docker.com/get-started/get-docker/):
```bash
$ docker build -t challenge .
$ docker run --rm -it challenge --auth=<token>
```


If rust `1.89` or later is locally installed, run the program directly for convenience:
```bash
$ cargo run -- --auth=<your token>
```

Additional command-line options are available:
- `--endpoint <url>`: Challenge server endpoint
- `--name <name>`: Problem name (optional)
- `--seed <seed>`: Problem seed (optional)
- `--rate <ms>`: Order placement rate in milliseconds (default: 500)
- `--min <seconds>`: Minimum pickup time in seconds (default: 4)
- `--max <seconds>`: Maximum pickup time in seconds (default: 8)

## Discard Criteria

When the shelf is full and a new order must be placed, the system selects the order to discard using a priority queue (minheap) ordered by expiration time. The order that expires earliest (or has already expired) is discarded.
