import os
from collections import Counter

import cv2
import numpy as np
from sklearn.cluster import DBSCAN

CHARACTERS = [
    "CAPTAIN_FALCON",
    "DONKEY_KONG",
    "FOX",
    "GAME_AND_WATCH",
    "KIRBY",
    "BOWSER",
    "LINK",
    "LUIGI",
    "MARIO",
    "MARTH",
    "MEWTWO",
    "NESS",
    "PEACH",
    "PIKACHU",
    "ICE_CLIMBERS",
    "JIGGLYPUFF",
    "SAMUS",
    "YOSHI",
    "ZELDA",
    "SHEIK",
    "FALCO",
    "YOUNG_LINK",
    "DR_MARIO",
    "ROY",
    "PICHU",
    "GANONDORF",
]


def get_dominant_color_dbscan(image_path, eps=10, min_samples=50):
    img = cv2.imread(image_path)
    if img is None:
        raise ValueError(f"Image not found or cannot be read: {image_path}")

    img_rgb = cv2.cvtColor(img, cv2.COLOR_BGR2RGB)
    pixels = img_rgb.reshape((-1, 3))

    clustering = DBSCAN(eps=eps, min_samples=min_samples, metric="euclidean")
    labels = clustering.fit_predict(pixels)

    filtered_labels = labels[labels != -1]
    filtered_pixels = pixels[labels != -1]

    if len(filtered_labels) == 0:
        raise ValueError(
            f"No clusters found in {image_path}. Try adjusting eps or min_samples."
        )

    counts = Counter(filtered_labels)
    dominant_label = counts.most_common(1)[0][0]
    dominant_cluster_pixels = filtered_pixels[filtered_labels == dominant_label]

    dominant_color = np.mean(dominant_cluster_pixels, axis=0).astype(int)
    return tuple(dominant_color)


def boost_vibrancy(rgb_color, saturation_scale=1.5, value_scale=1.2):
    # Convert RGB [0-255] to HSV [0-179,0-255,0-255]
    rgb = np.uint8([[rgb_color]])
    hsv = cv2.cvtColor(rgb, cv2.COLOR_RGB2HSV).astype(float)

    h, s, v = hsv[0, 0]

    # Boost saturation and value but clamp to max
    s = min(s * saturation_scale, 255)
    v = min(v * value_scale, 255)

    hsv[0, 0] = [h, s, v]

    # Convert back to RGB
    rgb_boosted = cv2.cvtColor(hsv.astype(np.uint8), cv2.COLOR_HSV2RGB)[0, 0]
    return tuple(rgb_boosted)


def rgb_to_hex(rgb):
    return "#{:02x}{:02x}{:02x}".format(*rgb)


def process_all_png(eps=10, min_samples=50):
    script_dir = os.path.dirname(os.path.abspath(__file__))
    # Map filenames (upper, without extension) to path
    png_files = {
        os.path.splitext(f)[0].upper(): os.path.join(script_dir, f)
        for f in os.listdir(script_dir)
        if f.lower().endswith(".png")
    }

    result_colors = []
    for character in CHARACTERS:
        path = png_files.get(character)
        if not path:
            print(f"Warning: No image file found for {character}")
            result_colors.append("#000000")  # fallback black
            continue
        try:
            dominant_color = get_dominant_color_dbscan(
                path, eps=eps, min_samples=min_samples
            )
            vibrant_color = boost_vibrancy(dominant_color)
            hex_color = rgb_to_hex(vibrant_color)
            result_colors.append(hex_color)
        except Exception as e:
            print(f"Error processing {character}: {e}")
            result_colors.append("#000000")  # fallback black

    print("const COLORS = [")
    for color in result_colors:
        print(f'    "{color}",')
    print("];")


if __name__ == "__main__":
    process_all_png()
