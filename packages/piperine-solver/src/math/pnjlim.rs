pub fn pnjlim(v_new: f64, v_old: f64, vt: f64, v_crit: f64) -> f64 {
    if (v_new > v_crit) && ((v_new - v_old).abs() > 2.0 * vt) {
        if v_old > 0.0 {
            let arg = 1.0 + (v_new - v_old) / vt;

            if arg > 0.0 {
                return v_old + vt * arg.ln();
            }
        } else {
            return v_crit;
        }
    }

    v_new
}
