# roxy (rust tcp/udp proxy)
I learned a lot making this, and I didn't expect it to be as fast as it is. I tested it proxying connections to game servers on a beaglebone with a 1ghz processor and 512mb of memory and encountered very little lag. Rust is awesome!

```
ROXY Help:

Command-line usage: roxy <rules> [--bind <address>] [--max-workers <#] [--debug]
Rule Syntax: --<tcp|udp> <incomming port>:<target host/ip>:<target port>

Example: roxy --tcp 8080:localhost:80
Example: roxy --udp 3443:192.0.0.1:2550 --bind 100.101.102.103
...
```
