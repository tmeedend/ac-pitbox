---
id: friend_call
type: call
from: friend
---
friend: Got your new phone? Good.
friend: The next race is in the calendar. Ready?
* (ready) [Ready.]
    friend: That's what I like to hear.
* (nervous) [A bit nervous.]
    ~ effect("rel.friend += 1")
    friend: Normal. I'll be in the pits.
