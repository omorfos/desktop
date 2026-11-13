### The words of the boxes that ask for a password or a passphrase, in
### English, which every other language falls back on a message at a time
### (ADR 79). What polkit asks for is polkit's, in the language it speaks.

# The administrator's password, asked for polkit.
polkit-heading = Authentication is required
polkit-go = Authenticate
# The password typed did not work, and the user may try again.
polkit-wrong = That did not work. Try again.

# A passphrase GnuPG asks for, of a key; a new one; and the new one again.
passphrase-heading = GnuPG asks for the passphrase of a key
passphrase-go = Unlock
new-heading = GnuPG asks for a new passphrase for a key
new-go = Go on
again-heading = Type the new passphrase again
again-go = Set
# The key, by its $fingerprint, as GnuPG lists it.
fingerprint = Fingerprint { $fingerprint }
# A key GnuPG gives no name.
unnamed = A key GnuPG does not name
# The passphrase GnuPG was given was not right.
not-right = That passphrase was not right.

# What the field is, where PAM asks for $user's password.
password-for = Password for { $user }
# What the eye beside the field does when pressed.
show-password = Show the password
hide-password = Hide the password
cancel = Cancel
