"""Render the clock's Earth animation: the Earth turning, the Moon orbiting it, and the Sun
at the right edge, pulsing, with sunspots. Full-panel frames, stacked from top to bottom in
a 24-bit BMP.

Usage (from the project root, needs Pillow):
    python3 tools/earth.py
Writes assets/clock/earth.bmp: FRAMES frames of WIDTH x HEIGHT, frame 0 at the top.
One loop = one Moon orbit = EARTH_TURNS turns of the Earth.

The right half of the panel gets the time and date over it (drawn by the app): the Moon's
orbit and the Sun stay in the free rows between them.

Land data: Natural Earth 1:110m land (public domain), from
https://github.com/nvkelso/natural-earth-vector/blob/master/geojson/ne_110m_land.geojson
Land colors: NASA Blue Marble (public domain), from
https://commons.wikimedia.org/wiki/File:Land_ocean_ice_2048.jpg
"""

import colorsys
import json
import math
import random
from pathlib import Path

from PIL import Image, ImageDraw

TOOLS_DIR = Path(__file__).parent
LAND_GEOJSON = TOOLS_DIR / "ne_110m_land.geojson"
LAND_COLORS_JPG = TOOLS_DIR / "land_ocean_ice_2048.jpg"
OUT_BMP = TOOLS_DIR.parent / "assets" / "clock" / "earth.bmp"

# Frames
WIDTH, HEIGHT = 64, 32  # the panel
SS = 4  # supersampling per axis (anti-aliasing)
FRAMES = 480  # one loop: 12 frames/s over 40 s in the app (EARTH_LOOP_PERIOD in clock.rs)
EARTH_TURNS = 2  # per Moon orbit: one turn every 20 s

# Earth, seen from 20°N 15°E (Europe and Africa), axis tilted like the real one
EARTH_CENTER = (16.0, 16.0)
EARTH_RADIUS = 15.0  # 30px tall
VIEW_LAT, VIEW_LON, AXIAL_TILT = 20, 15, 23.4

# Moon: orbit seen almost edge-on, to stay in the free rows between the time and the date
MOON_RADIUS = 3.2
MOON_ORBIT = (27.0, 2.5)  # half-axes of the ellipse, in pixels

# Sun: mostly off the panel, only its edge and glow show
SUN_CENTER = (66.0, 16.0)
SUN_RADIUS = 6.0
GLOW_WIDTH = 2.5  # how fast the glow fades, in pixels
# Pulse: the glow widens and the disc brightens, irregularly. Sine waves added together:
# (cycles per loop, weight, phase). Whole numbers of cycles, so that the loop doesn't jump;
# weights adding up to 1, so that the pulse stays within -1.0..1.0. Every 40-s loop: 14
# cycles = one every 2.9 s.
PULSE_WAVES = [(14, 0.5, 0.0), (22, 0.3, 2.1), (34, 0.2, 4.0)]
PULSE_GLOW = 0.3  # glow width change: +- this fraction
PULSE_DISC = 0.1  # disc brightness change: +- this fraction
# Sunspots on the Sun turning SUN_TURNS times per loop, left to right like the Earth.
# Only the left part of the disc is on the panel: each spot shows for ~1/5 of a turn.
SUN_TURNS = 2  # one turn every 20 s
SUNSPOTS = [(10, 0, 12), (-15, 140, 9), (25, 250, 8)]  # (lat, lon at t=0, radius), in degrees

# Saturated colors: they read better on LEDs than realistic ones
OCEAN = (0, 50, 190)
ICE = (235, 240, 255)
MOON = (200, 200, 190)
SUN = (255, 230, 90)
SUNSPOT = (120, 50, 0)
GLOW = (255, 150, 20)
ARCTIC_LAT = 75  # sea ice above this latitude
ANTARCTIC_LAT = -60

# Equirectangular land mask resolution (pixels per degree)
MASK_PPD = 4

# Land colors: the satellite map, made vivid for LEDs (see `vivid`)
LAND_COLORS_PPD = 2  # resolution once loaded (pixels per degree): plenty for a 30-px Earth
LAND_SATURATION = 2.0  # x the map's saturation
LAND_BRIGHTNESS = 1.4  # x the map's brightness
OLIVE_HUES = (50, 90)  # in degrees: olive greens look brown on LEDs...
OLIVE_TO_GREEN = 0.6  # ...so their hue moves toward green by this fraction

# Clouds: two random patterns of sine waves on the sphere (no seam at the poles or at the
# date line), mixed together differently along the loop, so that the clouds change shape.
# CLOUD_MORPHS cycles per loop: a whole number, so that the loop doesn't jump.
CLOUD = (255, 255, 255)
CLOUD_SEED = 1  # change it to get other clouds
CLOUD_WAVES = 24  # per pattern: more = more detailed clouds
CLOUD_FREQ = 12.0  # waves per radian, about: higher = smaller clouds
CLOUD_COVER = 0.25  # fraction of the map under clouds
CLOUD_EDGE = 0.6  # how soft the cloud edges are (in standard deviations of the patterns)
CLOUD_OPACITY = 0.9  # at the heart of a cloud
CLOUD_MORPHS = 1
CLOUD_PPD = 1  # resolution of the patterns (pixels per degree): they are smooth

# Sky: stars (single pixels at random places) and the Milky Way (a faint, clumpy band of
# light across the panel, with more, fainter stars in it)
STAR_SEED = 3  # change it to get other stars
STARS = 20  # scattered over the whole sky
STAR_BRIGHTNESS = (70, 150)  # range, out of 255
STAR_TINTS = [(1.0, 1.0, 1.0), (0.75, 0.85, 1.0), (1.0, 0.85, 0.6)]  # white, bluish, yellowish
MILKY_WAY = (45, 40, 70)  # color at the heart of the band
MILKY_WAY_LINE = ((-4.0, 30.0), (68.0, 0.0))  # the band's center line, from a point to another
MILKY_WAY_WIDTH = 5.0  # in pixels: how far from its line the band's light reaches
MILKY_WAY_STARS = 30  # extra stars in the band
MILKY_WAY_STAR_BRIGHTNESS = (40, 120)
MILKY_WAY_CLUMPS = 6  # random sine waves making the band's light uneven
# Where the app draws the time and the date (clock.rs): no stars there, so that the texts
# stay readable. (x0, y0, x1, y1), inclusive.
TEXT_BOXES = [(32, 4, 63, 12), (32, 19, 63, 27)]


def normalized(v):
    n = math.sqrt(sum(c * c for c in v))
    return tuple(c / n for c in v)


# Light from the Sun (right), a bit in front: the visible sides don't go fully dark
LIGHT = normalized((0.75, 0.15, 0.65))


def build_land_mask(path):
    """Rasterize Natural Earth land polygons; returns (land mask, ice mask)."""
    w, h = 360 * MASK_PPD, 180 * MASK_PPD
    land = Image.new("1", (w, h), 0)
    ice = Image.new("1", (w, h), 0)
    to_px = lambda p: ((p[0] + 180) * MASK_PPD, (90 - p[1]) * MASK_PPD)

    with open(path) as f:
        features = json.load(f)["features"]
    for feat in features:
        geom = feat["geometry"]
        polys = [geom["coordinates"]] if geom["type"] == "Polygon" else geom["coordinates"]
        for rings in polys:
            outer = [to_px(p) for p in rings[0]]
            # Greenland is an ice sheet: the polygon containing (-40, 72)
            lons = [p[0] for p in rings[0]]
            lats = [p[1] for p in rings[0]]
            is_greenland = min(lons) < -40 < max(lons) and min(lats) < 72 < max(lats) and max(lons) < 0
            ImageDraw.Draw(land).polygon(outer, fill=1)
            if is_greenland:
                ImageDraw.Draw(ice).polygon(outer, fill=1)
            for hole in rings[1:]:
                ImageDraw.Draw(land).polygon([to_px(p) for p in hole], fill=0)
    return land, ice


def vivid(color):
    """A satellite color made to read well on LEDs: more saturated and brighter, with the
    olive greens moved toward green."""
    h, s, v = colorsys.rgb_to_hsv(*(c / 255 for c in color))
    olive_min, olive_max = (hue / 360 for hue in OLIVE_HUES)
    if olive_min < h < olive_max:
        h += (h - olive_min) * OLIVE_TO_GREEN
    r, g, b = colorsys.hsv_to_rgb(h, min(1, s * LAND_SATURATION), min(1, v * LAND_BRIGHTNESS))
    return round(r * 255), round(g * 255), round(b * 255)


def build_land_colors(path):
    """Load the satellite map, at LAND_COLORS_PPD, with vivid colors: once for all frames."""
    img = Image.open(path).convert("RGB")
    # BOX: each pixel averages the map's pixels it covers
    img = img.resize((360 * LAND_COLORS_PPD, 180 * LAND_COLORS_PPD), Image.Resampling.BOX)
    img.putdata([vivid(c) for c in img.getdata()])
    return img


def lookup(mask, lat, lon):
    """The pixel of an equirectangular map (any resolution) at (lat, lon)."""
    w, h = mask.size
    x = int((lon + 180) % 360 * w / 360) % w
    y = min(int((90 - lat) * h / 180), h - 1)
    return mask.getpixel((x, y))


def surface_color(land, ice, land_colors, lat, lon):
    if lat > ARCTIC_LAT or lat < ANTARCTIC_LAT or lookup(ice, lat, lon):
        return ICE
    return lookup(land_colors, lat, lon) if lookup(land, lat, lon) else OCEAN


def cloud_pattern(seed):
    """A random cloud pattern: CLOUD_WAVES sine waves added together, in 3D so that the
    sphere has no seam. Its values (standard deviation ~1), row by row of an
    equirectangular map at CLOUD_PPD."""
    rng = random.Random(seed)
    waves = []
    for _ in range(CLOUD_WAVES):
        # A random direction (uniform on the sphere) and length (the wave's frequency)
        z, a = rng.uniform(-1, 1), rng.uniform(0, 2 * math.pi)
        f = CLOUD_FREQ * rng.uniform(0.6, 1.4)
        r = f * math.sqrt(1 - z * z)
        waves.append(((r * math.cos(a), r * math.sin(a), f * z), rng.uniform(0, 2 * math.pi)))
    norm = math.sqrt(CLOUD_WAVES / 2)  # a sine wave's variance is 1/2

    values = []
    for py in range(180 * CLOUD_PPD):
        lat = math.radians(90 - (py + 0.5) / CLOUD_PPD)
        for px in range(360 * CLOUD_PPD):
            lon = math.radians((px + 0.5) / CLOUD_PPD - 180)
            p = (math.cos(lat) * math.cos(lon), math.cos(lat) * math.sin(lon), math.sin(lat))
            value = sum(math.sin(sum(ki * pi for ki, pi in zip(k, p)) + phase) for k, phase in waves)
            values.append(value / norm)
    return values


def build_clouds():
    """Both cloud patterns as "F" (float) images, and the threshold above which there are
    clouds (CLOUD_COVER)."""
    a, b = cloud_pattern(CLOUD_SEED), cloud_pattern(CLOUD_SEED + 1)
    threshold = sorted(a)[int(len(a) * (1 - CLOUD_COVER))]
    images = []
    for values in (a, b):
        img = Image.new("F", (360 * CLOUD_PPD, 180 * CLOUD_PPD))
        img.putdata(values)
        images.append(img)
    return images[0], images[1], threshold


def with_clouds(color, clouds, morph, lat, lon):
    """`color` under the clouds at (lat, lon). `morph`: (cos, sin) of the patterns' mix."""
    a, b, threshold = clouds
    # cos² + sin² = 1: the mix keeps the patterns' spread, so the cover stays the same
    value = morph[0] * lookup(a, lat, lon) + morph[1] * lookup(b, lat, lon)
    alpha = CLOUD_OPACITY * min(1.0, max(0.0, (value - threshold) / CLOUD_EDGE))
    return tuple(c + (cloud - c) * alpha for c, cloud in zip(color, CLOUD))


def sphere_normal(x, y, center, radius):
    """Normal of a sphere at panel point (x, y) (x right, y up, z to the viewer), or None
    outside of it."""
    nx, ny = (x - center[0]) / radius, (center[1] - y) / radius
    rho2 = nx * nx + ny * ny
    if rho2 >= 1:
        return None
    return nx, ny, math.sqrt(1 - rho2)


def brightness(normal, ambient):
    """Lambert shading: `ambient` on the night side, 1.0 facing the Sun."""
    lit = sum(n * l for n, l in zip(normal, LIGHT))
    return ambient + (1 - ambient) * max(0.0, lit)


def earth_lat_lon(normal):
    """Inverse orthographic projection: (lat, lon) in degrees under a point of the Earth,
    the longitude relative to the view's one."""
    nx, ny, _ = normal
    phi0 = math.radians(VIEW_LAT)
    ct, st = math.cos(math.radians(AXIAL_TILT)), math.sin(math.radians(AXIAL_TILT))
    # Undo the axial tilt (image-plane rotation)
    gx, gy = nx * ct + ny * st, -nx * st + ny * ct
    rho = math.sqrt(nx * nx + ny * ny)
    if rho == 0:
        return VIEW_LAT, 0.0
    c = math.asin(rho)
    lat = math.asin(math.cos(c) * math.sin(phi0) + gy * math.sin(c) * math.cos(phi0) / rho)
    lon = math.atan2(gx * math.sin(c), rho * math.cos(c) * math.cos(phi0) - gy * math.sin(c) * math.sin(phi0))
    return math.degrees(lat), math.degrees(lon)


def sun_pulse(t):
    """The Sun's pulse at `t` (0.0..1.0 of the loop), -1.0..1.0: the sum of PULSE_WAVES."""
    return sum(w * math.sin(2 * math.pi * n * t + phase) for n, w, phase in PULSE_WAVES)


def sunspots_at(t):
    """Each sunspot at `t` (0.0..1.0 of the loop): its center on the Sun (a unit vector, in
    the axes of `sphere_normal`) and the cosine of its radius."""
    turn = 360 * SUN_TURNS * t  # increasing: the spots move left to right
    spots = []
    for lat, lon, radius in SUNSPOTS:
        lat, lon = math.radians(lat), math.radians(lon + turn)
        center = (math.cos(lat) * math.sin(lon), math.sin(lat), math.cos(lat) * math.cos(lon))
        spots.append((center, math.cos(math.radians(radius))))
    return spots


def sun_color(sun, pulse, spots):
    """The Sun's disc with its sunspots, or its glow fading with the distance.
    `sun`: (distance to the Sun's center, sphere normal or None), from `static_samples`;
    `pulse`: -1.0..1.0; `spots`: from `sunspots_at`."""
    d, normal = sun
    if normal:
        for center, cos_radius in spots:
            # Closer to the spot's center than its radius (the angle between them)
            if sum(n * c for n, c in zip(normal, center)) > cos_radius:
                return SUNSPOT
        return scale(SUN, 1 + PULSE_DISC * pulse)
    g = math.exp(-(d - SUN_RADIUS) / (GLOW_WIDTH * (1 + PULSE_GLOW * pulse)))
    return scale(GLOW, g)


def moon_position(t):
    """Center of the Moon at `t` (0.0..1.0 of the orbit), and whether it is in front of the
    Earth. Starts on the left; moves left to right in front, like the real one."""
    angle = 2 * math.pi * t
    x = EARTH_CENTER[0] - MOON_ORBIT[0] * math.cos(angle)
    y = EARTH_CENTER[1] + MOON_ORBIT[1] * math.sin(angle)
    return (x, y), math.sin(angle) > 0  # the near half of the orbit is the lower one


def scale(color, k):
    return tuple(c * k for c in color)


def add(color, other):
    return tuple(c + o for c, o in zip(color, other))


def milky_way_axes():
    """A point on the Milky Way's center line, the line's direction and its normal."""
    (x0, y0), (x1, y1) = MILKY_WAY_LINE
    along = normalized((x1 - x0, y1 - y0))
    return (x0, y0), along, (-along[1], along[0])


def build_sky():
    """The sky, computed once: the stars ({pixel: color}) and the Milky Way's clumps
    (random 2D sine waves)."""
    rng = random.Random(STAR_SEED)
    # Candidates (position, brightness range): scattered stars, then the Milky Way's
    candidates = []
    for _ in range(STARS):
        candidates.append(((rng.uniform(0, WIDTH), rng.uniform(0, HEIGHT)), STAR_BRIGHTNESS))
    origin, along, normal = milky_way_axes()
    for _ in range(MILKY_WAY_STARS):
        # Anywhere along the line, mostly close to it
        t, d = rng.uniform(0, math.dist(*MILKY_WAY_LINE)), rng.gauss(0, MILKY_WAY_WIDTH / 2)
        position = add(origin, add(scale(along, t), scale(normal, d)))
        candidates.append((position, MILKY_WAY_STAR_BRIGHTNESS))

    stars = {}
    for (x, y), (low, high) in candidates:
        px, py = int(x), int(y)
        in_text = any(x0 <= px <= x1 and y0 <= py <= y1 for x0, y0, x1, y1 in TEXT_BOXES)
        if 0 <= px < WIDTH and 0 <= py < HEIGHT and not in_text:
            stars[(px, py)] = scale(rng.choice(STAR_TINTS), rng.uniform(low, high))

    clumps = []
    for _ in range(MILKY_WAY_CLUMPS):
        # A random direction, and a wavelength of 4 to 10 pixels
        a, k = rng.uniform(0, 2 * math.pi), 2 * math.pi / rng.uniform(4, 10)
        clumps.append((k * math.cos(a), k * math.sin(a), rng.uniform(0, 2 * math.pi)))
    return stars, clumps


def milky_way_light(x, y, clumps):
    """The Milky Way's light at panel point (x, y): strongest on its line, fading with the
    distance, and uneven (`clumps`, from `build_sky`)."""
    origin, _, normal = milky_way_axes()
    d = (x - origin[0]) * normal[0] + (y - origin[1]) * normal[1]
    band = math.exp(-((d / MILKY_WAY_WIDTH) ** 2))
    waves = sum(math.sin(kx * x + ky * y + phase) for kx, ky, phase in clumps)
    # 0.0..1.0, mostly around 0.5: the waves' sum has a standard deviation of sqrt(n / 2)
    clumpy = min(1.0, max(0.0, 0.5 + 0.35 * waves / math.sqrt(len(clumps) / 2)))
    return scale(MILKY_WAY, band * (0.3 + 0.7 * clumpy))


def static_samples(sky):
    """What doesn't move, for every sample, computed once for all the frames: its pixel,
    its position, the Sun's (distance, sphere normal or None), the sky's color (`sky`:
    from `build_sky`) and the Earth's (lat, lon, brightness) or None."""
    stars, clumps = sky
    samples = []
    for py in range(HEIGHT):
        for px in range(WIDTH):
            for j in range(SS):
                for i in range(SS):
                    x, y = px + (i + 0.5) / SS, py + (j + 0.5) / SS
                    normal = sphere_normal(x, y, EARTH_CENTER, EARTH_RADIUS)
                    earth = None
                    if normal:
                        lat, lon = earth_lat_lon(normal)
                        earth = (lat, lon, brightness(normal, 0.25))
                    sun_distance = math.hypot(x - SUN_CENTER[0], y - SUN_CENTER[1])
                    sun = (sun_distance, sphere_normal(x, y, SUN_CENTER, SUN_RADIUS))
                    star = stars.get((px, py), (0, 0, 0))
                    sky_color = add(milky_way_light(x, y, clumps), star)
                    samples.append(((px, py), x, y, sun, sky_color, earth))
    return samples


def render_frame(samples, land, ice, land_colors, clouds, t):
    """The frame at `t` (0.0..1.0 of the loop): each pixel averages its samples."""
    earth_lon = VIEW_LON - EARTH_TURNS * 360 * t  # decreasing: turns west to east
    moon_center, moon_in_front = moon_position(t)
    # Once per frame, not per sample
    pulse = sun_pulse(t)
    spots = sunspots_at(t)
    angle = 2 * math.pi * CLOUD_MORPHS * t
    morph = (math.cos(angle), math.sin(angle))

    sums = {}
    for pixel, x, y, sun, sky, earth in samples:
        # Back to front: sky, Sun, Moon if behind, Earth, Moon if in front
        moon = None
        normal = sphere_normal(x, y, moon_center, MOON_RADIUS)
        if normal:
            moon = scale(MOON, brightness(normal, 0.12))
        color = sun_color(sun, pulse, spots)
        if not sun[1]:  # the Sun's disc hides the sky, its glow adds to it
            color = add(color, sky)
        if moon and not moon_in_front:
            color = moon
        if earth:
            lat, lon, k = earth
            lon += earth_lon
            color = surface_color(land, ice, land_colors, lat, lon)
            # Shaded with the surface: clouds go dark on the night side too
            color = scale(with_clouds(color, clouds, morph, lat, lon), k)
        if moon and moon_in_front:
            color = moon
        s = sums.setdefault(pixel, [0.0, 0.0, 0.0])
        for c in range(3):
            s[c] += color[c]

    img = Image.new("RGB", (WIDTH, HEIGHT), 0)
    for pixel, s in sums.items():
        img.putpixel(pixel, tuple(min(255, round(v / (SS * SS))) for v in s))
    return img


if __name__ == "__main__":
    land, ice = build_land_mask(LAND_GEOJSON)
    land_colors = build_land_colors(LAND_COLORS_JPG)
    clouds = build_clouds()
    samples = static_samples(build_sky())
    strip = Image.new("RGB", (WIDTH, HEIGHT * FRAMES), 0)
    for f in range(FRAMES):
        frame = render_frame(samples, land, ice, land_colors, clouds, f / FRAMES)
        strip.paste(frame, (0, f * HEIGHT))
    strip.save(OUT_BMP)  # PIL writes uncompressed 24-bit BMP for RGB images
    print(f"Written {OUT_BMP}: {FRAMES} frames of {WIDTH}x{HEIGHT}")
