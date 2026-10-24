---
id: talk_01
type: dialogue
characters: [friend]
scene: garage
---
friend: Face to face, no phone this time. #expression:happy #position:left
friend: Another race, or a break?
* (another) [Another race.]
    friend: That's the spirit. #expression:happy
* (rest) [A break.]
    ~ effect("rel.friend -= 1")
    friend: Fair enough. #expression:neutral #leave
