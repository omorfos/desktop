### The lock screen's words, in English, which every other language falls
### back on a message at a time (ADR 79). What PAM asks, as "Password:",
### is PAM's, in the language PAM is set to speak.

# The buttons along the bottom, and what each asks before it is done.
sleep = Sleep
restart = Restart
shut-down = Shut Down
log-out = Log Out
cancel = Cancel
restart-asked = Restart? What is not saved in open programs is lost.
shut-down-asked = Shut down? What is not saved in open programs is lost.
log-out-asked = Log out? What is not saved in open programs is lost.

# What was asked could not be done: $act is a button's words, and $why
# what the system said, in its own words.
did-not-happen = { $act } did not happen: { $why }

# A program keeps the machine from sleeping, or from turning off. $who is
# the program and, after a colon, why, in its own words.
kept-from = Kept from { $from ->
    [sleep] sleeping
   *[off] turning off
} by { $who }

# The password typed was not right.
wrong = That was not right

# A way of unlocking, as a key, asked for something to be typed, which
# only the password is.
types = asks for something to be typed, which only the password is

# A security key is waiting to be touched, as pam_u2f says in English.
touch-key = Touch the security key

# The reader is waiting for a finger, as pam_fprintd says in English, and
# a finger it read was not one it knows.
finger-place = Place a finger on the reader
finger-wrong = That finger was not recognised
