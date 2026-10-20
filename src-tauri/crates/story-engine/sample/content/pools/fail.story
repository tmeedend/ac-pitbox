---
id: fail_01_tease
type: message
from: friend
pool: fail_01
when: ["rel.friend >= warm"]
---
friend: [line] A teasing line, only between friends.
---
id: fail_01_close
type: message
from: friend
pool: fail_01
when: ["event.race_01.position == 4"]
---
friend: [line] So close.
---
id: fail_01_family
type: message
from: parent
pool: fail_01
---
parent: [line] An encouraging line.
---
id: fail_02_friend
type: message
from: friend
pool: fail_02
---
friend: [line] Next time.
---
id: fail_02_parent
type: message
from: parent
pool: fail_02
---
parent: [line] Proud of you anyway.
---
id: fail_02_rival
type: social_post
from: rival
pool: fail_02
when: ["rel.rival <= cold"]
---
rival: [line] As expected.
