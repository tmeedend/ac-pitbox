---
id: fail_01_tease
type: message
from: friend
pool: fail_01
when: ["rel.friend >= warm"]
---
friend: Was that a race or a Sunday drive?
---
id: fail_01_close
type: message
from: friend
pool: fail_01
when: ["event.race_01.position == 4"]
---
friend: So close.
---
id: fail_01_family
type: message
from: parent
pool: fail_01
---
parent: Keep at it, you are getting there.
---
id: fail_02_friend
type: message
from: friend
pool: fail_02
---
friend: Next time.
---
id: fail_02_parent
type: message
from: parent
pool: fail_02
---
parent: Proud of you anyway.
---
id: fail_02_rival
type: social_post
from: rival
pool: fail_02
when: ["rel.rival <= cold"]
---
rival: As expected.
