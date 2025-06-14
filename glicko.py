import numpy as np

TAU = 0.5
MU = 1500.0
SCALE = 173.7178


def g(phi):
    return 1 / np.sqrt(1 + 3 * (phi**2) / (np.pi**2))


def E(mu, mu_j, phi_j):
    return 1 / (1 + np.exp(-g(phi_j) * (mu - mu_j)))


def compute_variance(mu, opponents):
    v_inv = 0
    for mu_j, phi_j, s_j in opponents:
        E_j = E(mu, mu_j, phi_j)
        v_inv += (g(phi_j) ** 2) * E_j * (1 - E_j)
    return 1 / v_inv


def compute_delta(mu, phi, v, opponents):
    delta_sum = 0
    for mu_j, phi_j, s_j in opponents:
        E_j = E(mu, mu_j, phi_j)
        delta_sum += g(phi_j) * (s_j - E_j)
    return v * delta_sum


def update_volatility(phi, delta, v, sigma):
    a = np.log(sigma**2)
    tau = TAU
    epsilon = 1e-6

    def f(x):
        exp_x = np.exp(x)
        num = exp_x * (delta**2 - phi**2 - v - exp_x)
        den = 2 * (phi**2 + v + exp_x) ** 2
        return num / den - (x - a) / (tau**2)

    A = a
    B = None
    if delta**2 > phi**2 + v:
        B = np.log(delta**2 - phi**2 - v)
    else:
        B = a - 1.0

    fA = f(A)
    fB = f(B)

    while abs(B - A) > epsilon:
        C = A + (A - B) * fA / (fB - fA)
        fC = f(C)

        if fC * fB < 0:
            A = B
            fA = fB
        else:
            fA /= 2

        B = C
        fB = fC

    return np.exp(A / 2)


def f(x, delta, phi, v, a):
    exp_x = np.exp(x)
    num = exp_x * (delta**2 - phi**2 - v - exp_x)
    den = 2 * (phi**2 + v + exp_x) ** 2
    return num / den - (x - a) / (TAU**2)


def glicko2_rating_update(player_rating, game_results):
    mu = (player_rating["rating"] - MU) / SCALE
    phi = player_rating["rd"] / SCALE
    sigma = player_rating["volatility"]

    if not game_results:
        phi_star = np.sqrt(phi**2 + sigma**2)
        return {"rating": MU + SCALE * mu, "rd": SCALE * phi_star, "volatility": sigma}

    opponents = []
    for result in game_results:
        opp_mu = (result["opponent_rating"] - MU) / SCALE
        opp_phi = result["opponent_rd"] / SCALE
        s = result["score"]
        opponents.append((opp_mu, opp_phi, s))

    v = compute_variance(mu, opponents)
    delta = compute_delta(mu, phi, v, opponents)
    sigma_prime = update_volatility(phi, delta, v, sigma)
    phi_star = np.sqrt(phi**2 + sigma_prime**2)
    phi_prime = 1 / np.sqrt(1 / (phi_star**2) + 1 / v)

    sum_term = 0
    for mu_j, phi_j, s_j in opponents:
        E_j = E(mu, mu_j, phi_j)
        sum_term += g(phi_j) * (s_j - E_j)

    mu_prime = mu + phi_prime**2 * sum_term

    return {
        "rating": MU + SCALE * mu_prime,
        "rd": SCALE * phi_prime,
        "volatility": sigma_prime,
    }


if __name__ == "__main__":
    # test should result in {'rating': 1464.06, 'rd': 151.52, 'volatility': 0.05999}
    player_rating = {"rating": 1500, "rd": 200, "volatility": 0.06}

    game_results = [
        {"opponent_rating": 1400, "opponent_rd": 30, "score": 1},
        {"opponent_rating": 1550, "opponent_rd": 100, "score": 0},
        {"opponent_rating": 1700, "opponent_rd": 300, "score": 0},
    ]

    updated_rating = glicko2_rating_update(player_rating, game_results)
    print(updated_rating)
