# Guac light client

Guac is structured into 3 modules, guac_core which consists of the implementation of the channel
update logic as well as interaction with the blockchain through bounty hunters. guac_actix is an
interface over guac_core which sends and receives messages from the external world via HTTP, but
receives commands via actix. guac_http makes the command interface HTTP instead to provide wider
interoperability.