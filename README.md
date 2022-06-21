# RBattle

**This repo is archived.** This depends on crates with security
vulnerabilities, and I don't have time to update them at the moment.
It also is so old that it doesn't run any more.

RBattle is a resurrection of the '90s-era X Windows game XBattle, written in Rust.
It uses the Glium OpenGL bindings for graphics, and Tokio for some of its networking.

To play, make sure the players' computers can connect to each other via TCP/IP.
(Many wifi networks don't permit associated hosts to communicate with each other
directly, only with the outside world.)

On one computer, run the command:

    $ cargo run server 0.0.0.0:12345

where `0.0.0.0:12345` give the IP address and TCP port the server should listen
for client connections on. Then, on up to three other computers, run:

    $ cargo run client ADDR:PORT

where `ADDR` is the IP address of the computer running the server, and `PORT` is
the same port number given to the server. The clients simply join the game in
progress, with each incoming client assigned to a different color.

When the game is started, each player owns a goop source. Click within squares
to toggle outflow pumps. Equal amounts of goop of different colors cancel each
other when they come in contact. Win by destroying all of your opponents' goop.

RBattle was written in haste. There are many, many improvements possible, and
the code is not great in some parts. There are surely plenty of bugs as well.
Pull requests are welcome!

# To do

- Mark goop sources.
- Animate flow
- Animate combat

# License

RBattle is copyright 2017, the RBattle developers. RBattle is distributed under
the terms of both the MIT license and the Apache License (Version 2.0), with
portions covered by various BSD-like licenses.

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT).
