# Brief

Stage 1. Sam's request, word for word, and the design it points to.

## The request (2026-10-10)

> Please use the Builder repo as a base in the directory above, and use the work and repos of Sam gilson at sgilson7.github.io, which can all be found one directory up. please build out the MVP and deploy it to my website. you have approval for creating a public github repo, github pages, and approval to deploy to my website when its ready. any time you need to make 2D art and any kind of asset for the game, use the tikz latex prompt you can find in other repos, and use sam comm final as an inspiration image, alongside the menu screens of all of the sgilson7 games which use a similar build pipeline for images. now build the mvp for the game and deploy it, dont stop till its deployed, good luck. here is the prompt from my friend to follow to create the game containing all design decisions: the game should be 3d.

## The design, from a friend of Sam's

> Game Idea: Battle Evolution Roguelike
>
> You travel along a branching path with various encounters (Battles, Random encounters, Shops etc)
> Initial Creatures have space for 3 Body Parts.
>
> Certain Random encounters can grant you additional "Mass", which increase the cap an additional body part.
> Other encounters may chop off mass, or add or remove body parts. Encounters are balanced to be 50% Positive and 50% negative.
>
> Battles may occur against other creatures ranging from Bugs and insects to crustaceans to common woodland creatures. Each creature contains 2-4 unique body parts, each providing an ability.
>
> An Example:
> Your opponent is a rabbit with 3 body parts on a body with 3 mass.
> Rabbit:
> Body Part 1: Bunny Ears: Grants 'Hearing': Ranged Abilities deal 1 less damage
> Body Part 2: Teeth: Deal 2 Melee damage every 2 seconds
> Body Part 3: Lucky feet: Grants +1 Evasion (1% Dodge Chance) every 5 seconds
>
> Another Example is a Wasp
>
> Wasp:
> Body Part 1: Wings: Grants +3 Evasion for each empty mass slot
> Body Part 2: Stinger: every 4 seconds, deal 1 poison stack that stacks up to 10. Each stack deals 1 damage every 2 seconds
>
> Creatures with more mass have debuffs that make their body parts activate slower.
> Creatures with more than 5 mass take a 10% slow debuff for each additional piece of mass and body parts above that number.
>
> Combat amongst creatures functions as an auto battler, with various items counting down until they activate, until 1 creature dies.
> Upon winning a battle, creatures can choose to increase their max health, or can choose up to 1 of the slain creatures body parts to adopt and place onto their own creature

(Spacing after colons normalised; the words are the friend's.)

## Playtest edits, from the game's designer (2026-10-11)

Sam passed these on after a play test, asking for them to be made, the game updated and deployed. Word for word:

> A couple of edits to the gameplay loop:
> -Players should be able to drag and drop the body parts onto the creature, something similar to how spore allows players to build creatures.
>
> -Players should be able to hold up to 2 Body parts in their inventory, body parts are inert and do not affect battle while in the inventory.
>
> -Add encounter to upgrade a body part
> -lower cost of additional mass in stores to 20 Acorns
> -Add the following Creatures to the pool and create 2-3 associated key features for each creature to add to the body part pool: Frog, Salmon, Bear, Bee swarm, Anthill, squirrel, skunk,possum, Hawk,turkey, Deer,Jackrabbit,Unicorn, Dragon Hatchling, snake, snail, Turtle,Duck,Swan.
>
> Add the following random Body parts that can modify any creature: Fungal Antenna, Third Eye, Opposable thumb,Prehensile tail,EyeStalk,Fur Patch,
> Two-Headed:Grant an extra mass slot that only a head item can fit in
> -Add an ability to "forage" after a battle, to fill up a forage bar and allow you to cultivate mass every 3-4 battles.
>
> Add the Following Mechanics, and create new body part to match if need be:limited health regeneration, no more than 3/sec
> Add set types: Bug, Rodent, Beast, Critter, assign these to each creature and their associated body parts.
> Add Set bonus for having multiples of each

## Autonomy

Sam approved in advance: a public GitHub repository, GitHub Pages for it, deploys to Pages, and adding the game to his website (sgilson7.github.io) when it is ready.

## Done means

The MVP plays in a browser from start to finish (choose a creature, walk the branching path, fight, adopt parts, meet encounters, shop, rest, beat or lose to the boss), the Builder check passes, and the game is live on Pages and listed on Sam's website.
