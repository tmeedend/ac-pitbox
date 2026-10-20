---
id: invite_01
type: message
from: friend
clock: "09:12"
---
friend: [line] There is a race next weekend.
friend: [line] Want to come?
* (accept) [Count me in.]
    ~ effect("rel.friend += 1")
* (decline) [Not this time.]
---
id: invite_02
type: message
from: friend
---
friend: [line] Come anyway, I signed you up.
---
id: congrats_01
type: message
from: friend
---
friend: [line] Told you so.
