# MatLED



# Notes

Not consistent between gtasks and weather. In gtask I'm passing a mutable class and I'm modifying it directly, while I'm cloning it in weather.


Error handling, (if empty object or error for instance).

On the RPi SD card, add:
- `/in` folder at the root with a Readme asking for the OAuth2 credentials and a `conf.txt` file containing enabled states.
- `/log` folder containing logs -> add logging feature.

(Optionnal)
- Add a ftp (or onther technology) server to be able to access config file and logs remotely on the local network.
- Add a mini http-server reachable from a machine on the same local network. Enable/Disable of features, logging.

## OAUTH2 Token Handling

On the final version, we would like to do the token creation on a PC, and then just forward the token files to the embedded board where this program will run, in `/src/app/res/credentials.json` and `/src/app/res/tokencache.json`.


Put OAuth2 authenticator in Context, because it might be needed to share it between different states in the future.
