# MatLED



# Notes

Not consistent between gtasks and weather. In gtask I pass a mutable class and I modify it directly, while I'm cloning it in weather.

## OAUTH2 Token Handling

On the final version, we would like to do the token creation on a PC, and then just forward the token files to the embedded board where this program will run, in `/src/app/res/credentials.json` and `/src/app/res/tokencache.json`.

Either only return authenticator in GTASKS init and then use it to get the token, or just don't call it in init, but as it is now I'm calling 2 times get_oauth2_google_token for nothing.

Mettre le authenticator et le token dans contexte plutot que dans GTASKS ?
