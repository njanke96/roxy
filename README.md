# roxy
A rust learning experience.

```
ROXY Help:

Command-line usage: roxy <rules> [--bind <address>] [--max-workers <#] [--debug]
Rule Syntax: --<tcp|udp> <incomming port>:<target host/ip>:<target port>

Example: roxy --tcp 8080:localhost:80
Example: roxy --udp 3443:192.0.0.1:2550 --bind 100.101.102.103

Note: UDP rules can only support one client per proxy port. Specify multiple rules if you need to support multiple bi-directional UDP channels.
roxy: No rules specified.
```
