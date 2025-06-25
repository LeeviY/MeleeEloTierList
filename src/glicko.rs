const TAU: f64 = 1.0;
const MU: f64 = 1500.0;
const SCALE: f64 = 173.7178;

#[derive(Debug, Clone, serde::Serialize)]
pub struct Player {
    pub rating: f64,
    pub rd: f64,
    pub volatility: f64,
    pub matches: i32,
}

#[derive(Debug, Clone)]
pub struct Opponent {
    pub rating: f64,
    pub rd: f64,
    pub score: f64,
}

fn g(phi: f64) -> f64 {
    1.0 / (1.0 + 3.0 * phi.powi(2) / std::f64::consts::PI.powi(2)).sqrt()
}

fn e(mu: f64, mu_j: f64, phi_j: f64) -> f64 {
    1.0 / (1.0 + (-g(phi_j) * (mu - mu_j)).exp())
}

fn compute_variance(mu: f64, opponents: &[(f64, f64, f64)]) -> f64 {
    let mut v_inv = 0.0;
    for &(mu_j, phi_j, _s_j) in opponents {
        let e_j = e(mu, mu_j, phi_j);
        v_inv += g(phi_j).powi(2) * e_j * (1.0 - e_j);
    }
    1.0 / v_inv
}

fn compute_delta(mu: f64, v: f64, opponents: &[(f64, f64, f64)]) -> f64 {
    let mut delta_sum = 0.0;
    for &(mu_j, phi_j, s_j) in opponents {
        let e_j = e(mu, mu_j, phi_j);
        delta_sum += g(phi_j) * (s_j - e_j);
    }
    v * delta_sum
}

fn f_function(x: f64, delta: f64, phi: f64, v: f64, a: f64) -> f64 {
    let exp_x = x.exp();
    let num = exp_x * (delta.powi(2) - phi.powi(2) - v - exp_x);
    let den = 2.0 * (phi.powi(2) + v + exp_x).powi(2);
    num / den - (x - a) / TAU.powi(2)
}

fn update_volatility(phi: f64, delta: f64, v: f64, sigma: f64) -> f64 {
    let a = (sigma.powi(2)).ln();
    let epsilon = 1e-6;

    let mut a_val = a;
    let mut b_val = if delta.powi(2) > phi.powi(2) + v {
        (delta.powi(2) - phi.powi(2) - v).ln()
    } else {
        a - 1.0
    };

    let mut fa = f_function(a_val, delta, phi, v, a);
    let mut fb = f_function(b_val, delta, phi, v, a);

    while (b_val - a_val).abs() > epsilon {
        let c = a_val + (a_val - b_val) * fa / (fb - fa);
        let fc = f_function(c, delta, phi, v, a);

        if fc * fb < 0.0 {
            a_val = b_val;
            fa = fb;
        } else {
            fa /= 2.0;
        }

        b_val = c;
        fb = fc;
    }

    (a_val / 2.0).exp()
}

impl Player {
    pub fn update(&self, game_results: &[Opponent]) -> Player {
        let mu = (self.rating - MU) / SCALE;
        let phi = self.rd / SCALE;
        let sigma = self.volatility;

        if game_results.is_empty() {
            let phi_star = (phi.powi(2) + sigma.powi(2)).sqrt();
            return Player {
                rating: MU + SCALE * mu,
                rd: SCALE * phi_star,
                volatility: sigma,
                matches: self.matches,
            };
        }

        let opponents: Vec<(f64, f64, f64)> = game_results
            .iter()
            .map(|result| {
                let opp_mu = (result.rating - MU) / SCALE;
                let opp_phi = result.rd / SCALE;
                let s = result.score;
                (opp_mu, opp_phi, s)
            })
            .collect();

        let v = compute_variance(mu, &opponents);
        let delta = compute_delta(mu, v, &opponents);
        let sigma_prime = update_volatility(phi, delta, v, sigma);
        let phi_star = (phi.powi(2) + sigma_prime.powi(2)).sqrt();
        let phi_prime = 1.0 / (1.0 / phi_star.powi(2) + 1.0 / v).sqrt();

        let mut sum_term = 0.0;
        for &(mu_j, phi_j, s_j) in &opponents {
            let e_j = e(mu, mu_j, phi_j);
            sum_term += g(phi_j) * (s_j - e_j);
        }

        let mu_prime = mu + phi_prime.powi(2) * sum_term;

        Player {
            rating: MU + SCALE * mu_prime,
            rd: SCALE * phi_prime,
            volatility: sigma_prime,
            matches: self.matches + game_results.len() as i32,
        }
    }

    pub fn update_mut(&mut self, game_results: &[Opponent]) {
        *self = self.update(game_results);
    }
}

pub fn win_probability(rating_a: f64, rating_b: f64, rd_b: f64) -> f64 {
    let mu_a = (rating_a - MU) / SCALE;
    let mu_b = (rating_b - MU) / SCALE;
    let phi_b = rd_b / SCALE;

    let g_phi = 1.0 / (1.0 + 3.0 * phi_b.powi(2) / std::f64::consts::PI.powi(2)).sqrt();
    let exponent = -g_phi * (mu_a - mu_b);
    let e_a = 1.0 / (1.0 + exponent.exp());

    e_a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rating_update() {
        let player_rating = Player {
            rating: 1500.0,
            rd: 200.0,
            volatility: 0.06,
            matches: 0,
        };

        let game_results = vec![
            Opponent {
                rating: 1400.0,
                rd: 30.0,
                score: 1.0,
            },
            Opponent {
                rating: 1550.0,
                rd: 100.0,
                score: 0.0,
            },
            Opponent {
                rating: 1700.0,
                rd: 300.0,
                score: 0.0,
            },
        ];

        let updated_rating = player_rating.update(&game_results);

        // Expected result: {'rating': 1464.06, 'rd': 151.52, 'volatility': 0.05999}
        println!("Updated rating: {:?}", updated_rating);
        assert!((updated_rating.rating - 1464.06).abs() < 0.1);
        assert!((updated_rating.rd - 151.52).abs() < 0.1);
        assert!((updated_rating.volatility - 0.05999).abs() < 0.001);
    }

    #[test]
    fn test_win_probability() {
        let prob = win_probability(1122.0, 1976.0, 111.0);
        println!("Win probability: {}", prob);
    }
}
