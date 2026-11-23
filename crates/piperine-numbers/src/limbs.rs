//! `Limbs`: magnitude arithmetic on little-endian `u64` limbs.
//!
//! A magnitude is a slice of limbs, least significant first. Results are returned *normalized*
//! (no high zero limb; zero is the empty vector); operands may carry high zero limbs. Division is
//! Knuth, TAOCP vol. 2 section 4.3.1, algorithm D.

use core::cmp::Ordering;

/// Arithmetic on a magnitude held as little-endian `u64` limbs.
pub trait Limbs {
    /// The limbs without high zero limbs (zero is the empty vector).
    fn trimmed(&self) -> Vec<u64>;
    /// Whether every limb is zero.
    fn is_zero_magnitude(&self) -> bool;
    /// The number of significant bits (0 for zero).
    fn bit_length(&self) -> u64;
    /// The number of trailing zero bits (0 for zero).
    fn trailing_zero_bits(&self) -> u64;
    /// The bit at `index` counted from the least significant bit (false past the end).
    fn bit(&self, index: u64) -> bool;
    /// Whether any bit below `index` is set.
    fn any_bit_below(&self, index: u64) -> bool;
    /// Compares two magnitudes as integers, ignoring high zero limbs.
    fn compare_magnitude(&self, other: &[u64]) -> Ordering;
    /// Adds `other` into `self` in place (`self` at least as long as `other`); the carry out.
    fn add_assign_limbs(&mut self, other: &[u64]) -> bool;
    /// Subtracts `other` from `self` in place (`self` at least as long as `other`); the borrow out.
    fn sub_assign_limbs(&mut self, other: &[u64]) -> bool;
    /// The sum of two magnitudes.
    fn plus(&self, other: &[u64]) -> Vec<u64>;
    /// The difference `self - other`, or `None` when `other` is larger.
    fn minus(&self, other: &[u64]) -> Option<Vec<u64>>;
    /// The product of two magnitudes (schoolbook).
    fn times(&self, other: &[u64]) -> Vec<u64>;
    /// The product with a single limb, plus a single-limb addend.
    fn times_small_plus(&self, factor: u64, addend: u64) -> Vec<u64>;
    /// Quotient and remainder by a single non-zero limb (`None` when the divisor is zero).
    fn divide_small(&self, divisor: u64) -> Option<(Vec<u64>, u64)>;
    /// Quotient and remainder by a magnitude (`None` when the divisor is zero).
    fn divide_with_remainder(&self, divisor: &[u64]) -> Option<(Vec<u64>, Vec<u64>)>;
    /// `self << count`, growing as needed.
    fn shifted_left(&self, count: u64) -> Vec<u64>;
    /// `self >> count`, dropping the shifted-out bits.
    fn shifted_right(&self, count: u64) -> Vec<u64>;
    /// `self << count` within the existing length, discarding what leaves the top.
    fn shift_left_assign(&mut self, count: u64);
    /// `self >> count` within the existing length, filling the top with `fill` bits.
    fn shift_right_assign(&mut self, count: u64, fill: bool);
    /// The floor of the square root, and whether the root is exact.
    fn integer_square_root(&self) -> (Vec<u64>, bool);
    /// The magnitude as decimal digits.
    fn to_decimal_string(&self) -> String;
    /// The magnitude as a `u128`, or `None` when it needs more than 128 bits.
    fn to_u128(&self) -> Option<u128>;
}

/// A fixed-width integer turned into a normalized limb vector.
pub trait ToLimbs {
    /// The value as a normalized magnitude.
    fn to_limbs(self) -> Vec<u64>;
}

impl ToLimbs for u64 {
    fn to_limbs(self) -> Vec<u64> {
        [self].trimmed()
    }
}

impl ToLimbs for u128 {
    fn to_limbs(self) -> Vec<u64> {
        [self as u64, (self >> 64) as u64].trimmed()
    }
}

impl Limbs for [u64] {
    fn trimmed(&self) -> Vec<u64> {
        let mut length = self.len();
        while length > 0 && self[length - 1] == 0 {
            length -= 1;
        }
        self[..length].to_vec()
    }

    fn is_zero_magnitude(&self) -> bool {
        self.iter().all(|&limb| limb == 0)
    }

    fn bit_length(&self) -> u64 {
        for index in (0..self.len()).rev() {
            if self[index] != 0 {
                return index as u64 * 64 + (64 - self[index].leading_zeros() as u64);
            }
        }
        0
    }

    fn trailing_zero_bits(&self) -> u64 {
        for (index, limb) in self.iter().enumerate() {
            if *limb != 0 {
                return index as u64 * 64 + limb.trailing_zeros() as u64;
            }
        }
        0
    }

    fn bit(&self, index: u64) -> bool {
        let limb = (index / 64) as usize;
        limb < self.len() && (self[limb] >> (index % 64)) & 1 == 1
    }

    fn any_bit_below(&self, index: u64) -> bool {
        let whole = ((index / 64) as usize).min(self.len());
        if self[..whole].iter().any(|&limb| limb != 0) {
            return true;
        }
        let rest = index % 64;
        whole < self.len() && rest > 0 && self[whole] & ((1u64 << rest) - 1) != 0
    }

    fn compare_magnitude(&self, other: &[u64]) -> Ordering {
        let longest = self.len().max(other.len());
        for index in (0..longest).rev() {
            let left = self.get(index).copied().unwrap_or(0);
            let right = other.get(index).copied().unwrap_or(0);
            if left != right {
                return left.cmp(&right);
            }
        }
        Ordering::Equal
    }

    fn add_assign_limbs(&mut self, other: &[u64]) -> bool {
        let mut carry = false;
        for index in 0..self.len() {
            let addend = other.get(index).copied().unwrap_or(0);
            let (first, first_carry) = self[index].overflowing_add(addend);
            let (second, second_carry) = first.overflowing_add(carry as u64);
            self[index] = second;
            carry = first_carry || second_carry;
        }
        carry
    }

    fn sub_assign_limbs(&mut self, other: &[u64]) -> bool {
        let mut borrow = false;
        for index in 0..self.len() {
            let subtrahend = other.get(index).copied().unwrap_or(0);
            let (first, first_borrow) = self[index].overflowing_sub(subtrahend);
            let (second, second_borrow) = first.overflowing_sub(borrow as u64);
            self[index] = second;
            borrow = first_borrow || second_borrow;
        }
        borrow
    }

    fn plus(&self, other: &[u64]) -> Vec<u64> {
        let (longer, shorter) = if self.len() >= other.len() { (self, other) } else { (other, self) };
        let mut sum = longer.to_vec();
        sum.push(0);
        sum.add_assign_limbs(shorter);
        sum.trimmed()
    }

    fn minus(&self, other: &[u64]) -> Option<Vec<u64>> {
        if self.compare_magnitude(other) == Ordering::Less {
            return None;
        }
        let mut difference = self.to_vec();
        difference.sub_assign_limbs(other);
        Some(difference.trimmed())
    }

    fn times(&self, other: &[u64]) -> Vec<u64> {
        let mut product = vec![0u64; self.len() + other.len()];
        for (left_index, &left) in self.iter().enumerate() {
            let mut carry = 0u128;
            for (right_index, &right) in other.iter().enumerate() {
                let cell = product[left_index + right_index] as u128 + left as u128 * right as u128 + carry;
                product[left_index + right_index] = cell as u64;
                carry = cell >> 64;
            }
            product[left_index + other.len()] = carry as u64;
        }
        product.trimmed()
    }

    fn times_small_plus(&self, factor: u64, addend: u64) -> Vec<u64> {
        let mut result = Vec::with_capacity(self.len() + 1);
        let mut carry = addend as u128;
        for &limb in self {
            let cell = limb as u128 * factor as u128 + carry;
            result.push(cell as u64);
            carry = cell >> 64;
        }
        result.push(carry as u64);
        result.trimmed()
    }

    fn divide_small(&self, divisor: u64) -> Option<(Vec<u64>, u64)> {
        if divisor == 0 {
            return None;
        }
        let mut quotient = vec![0u64; self.len()];
        let mut remainder = 0u128;
        for index in (0..self.len()).rev() {
            let cell = (remainder << 64) | self[index] as u128;
            quotient[index] = (cell / divisor as u128) as u64;
            remainder = cell % divisor as u128;
        }
        Some((quotient.trimmed(), remainder as u64))
    }

    fn divide_with_remainder(&self, divisor: &[u64]) -> Option<(Vec<u64>, Vec<u64>)> {
        let denominator = divisor.trimmed();
        let numerator = self.trimmed();
        if denominator.is_empty() {
            return None;
        }
        if numerator.compare_magnitude(&denominator) == Ordering::Less {
            return Some((Vec::new(), numerator));
        }
        if denominator.len() == 1 {
            let (quotient, remainder) = numerator.divide_small(denominator[0])?;
            return Some((quotient, remainder.to_limbs()));
        }
        Some(KnuthDivision::new(&numerator, &denominator).run())
    }

    fn shifted_left(&self, count: u64) -> Vec<u64> {
        let whole = (count / 64) as usize;
        let bits = count % 64;
        let mut result = vec![0u64; whole];
        let mut carry = 0u64;
        for &limb in self {
            result.push(if bits == 0 { limb } else { (limb << bits) | carry });
            carry = if bits == 0 { 0 } else { limb >> (64 - bits) };
        }
        result.push(carry);
        result.trimmed()
    }

    fn shifted_right(&self, count: u64) -> Vec<u64> {
        let whole = (count / 64) as usize;
        let bits = count % 64;
        if whole >= self.len() {
            return Vec::new();
        }
        let mut result = Vec::with_capacity(self.len() - whole);
        for index in whole..self.len() {
            let high = self.get(index + 1).copied().unwrap_or(0);
            result.push(if bits == 0 { self[index] } else { (self[index] >> bits) | (high << (64 - bits)) });
        }
        result.trimmed()
    }

    fn shift_left_assign(&mut self, count: u64) {
        let length = self.len();
        let whole = (count / 64).min(length as u64) as usize;
        let bits = count % 64;
        for index in (0..length).rev() {
            let source = index.checked_sub(whole);
            let low = source.map(|position| self[position]).unwrap_or(0);
            let below = source.and_then(|position| position.checked_sub(1)).map(|position| self[position]).unwrap_or(0);
            self[index] = if bits == 0 { low } else { (low << bits) | (below >> (64 - bits)) };
        }
    }

    fn shift_right_assign(&mut self, count: u64, fill: bool) {
        let length = self.len();
        let whole = (count / 64).min(length as u64) as usize;
        let bits = count % 64;
        let filler = if fill { u64::MAX } else { 0 };
        for index in 0..length {
            let low = self.get(index + whole).copied().unwrap_or(filler);
            let high = self.get(index + whole + 1).copied().unwrap_or(filler);
            self[index] = if bits == 0 { low } else { (low >> bits) | (high << (64 - bits)) };
        }
    }

    fn integer_square_root(&self) -> (Vec<u64>, bool) {
        let number = self.trimmed();
        if number.is_empty() {
            return (Vec::new(), true);
        }
        // Newton's iteration from above: the sequence decreases monotonically to the floor root.
        let mut root = 1u64.to_limbs().shifted_left(number.bit_length().div_ceil(2));
        loop {
            let quotient = number.divide_with_remainder(&root).map(|(quotient, _)| quotient).unwrap_or_default();
            let next = root.plus(&quotient).shifted_right(1);
            if next.compare_magnitude(&root) != Ordering::Less {
                break;
            }
            root = next;
        }
        let exact = root.times(&root).compare_magnitude(&number) == Ordering::Equal;
        (root, exact)
    }

    fn to_decimal_string(&self) -> String {
        let mut rest = self.trimmed();
        if rest.is_empty() {
            return "0".to_string();
        }
        let mut chunks: Vec<u64> = Vec::new();
        while !rest.is_empty() {
            let (quotient, remainder) = rest.divide_small(10_000_000_000_000_000_000).unwrap_or_default();
            chunks.push(remainder);
            rest = quotient;
        }
        let mut text = chunks.pop().map(|chunk| chunk.to_string()).unwrap_or_default();
        while let Some(chunk) = chunks.pop() {
            text.push_str(&format!("{chunk:019}"));
        }
        text
    }

    fn to_u128(&self) -> Option<u128> {
        let trimmed = self.trimmed();
        match trimmed.len() {
            0 => Some(0),
            1 => Some(trimmed[0] as u128),
            2 => Some(trimmed[0] as u128 | (trimmed[1] as u128) << 64),
            _ => None,
        }
    }
}

/// One run of Knuth's algorithm D over a numerator and a denominator of at least two limbs.
struct KnuthDivision {
    /// The normalized numerator, one limb longer than the original.
    numerator: Vec<u64>,
    /// The normalized denominator (top bit set).
    denominator: Vec<u64>,
    /// The normalization shift in bits.
    shift: u64,
}

impl KnuthDivision {
    /// Normalizes the operands so the denominator's top bit is set (step D1).
    fn new(numerator: &[u64], denominator: &[u64]) -> Self {
        let shift = denominator[denominator.len() - 1].leading_zeros() as u64;
        let mut shifted_numerator = numerator.to_vec();
        shifted_numerator.push(0);
        shifted_numerator.shift_left_assign(shift);
        let mut shifted_denominator = denominator.to_vec();
        shifted_denominator.shift_left_assign(shift);
        KnuthDivision { numerator: shifted_numerator, denominator: shifted_denominator, shift }
    }

    /// Runs steps D2 to D8 and returns the normalized quotient and remainder.
    fn run(mut self) -> (Vec<u64>, Vec<u64>) {
        let divisor_length = self.denominator.len();
        let quotient_length = self.numerator.len() - divisor_length;
        let mut quotient = vec![0u64; quotient_length];
        for position in (0..quotient_length).rev() {
            quotient[position] = self.quotient_digit(position);
        }
        let mut remainder = self.numerator[..divisor_length].to_vec();
        remainder.shift_right_assign(self.shift, false);
        (quotient.trimmed(), remainder.trimmed())
    }

    /// One quotient digit (D3 estimate, D4 multiply and subtract, D5 to D6 add back).
    fn quotient_digit(&mut self, position: usize) -> u64 {
        let length = self.denominator.len();
        let top = self.denominator[length - 1] as u128;
        let second = self.denominator[length - 2] as u128;
        let numerator_top = (self.numerator[position + length] as u128) << 64 | self.numerator[position + length - 1] as u128;
        let mut estimate = numerator_top / top;
        let mut remainder = numerator_top % top;
        while estimate >> 64 != 0 || estimate * second > (remainder << 64 | self.numerator[position + length - 2] as u128) {
            estimate -= 1;
            remainder += top;
            if remainder >> 64 != 0 {
                break;
            }
        }
        let negative = self.subtract_multiple(position, estimate as u64);
        if negative {
            self.add_back(position);
            return (estimate as u64).wrapping_sub(1);
        }
        estimate as u64
    }

    /// Subtracts `digit * denominator` from the numerator window at `position`; true on a borrow.
    fn subtract_multiple(&mut self, position: usize, digit: u64) -> bool {
        let length = self.denominator.len();
        let mut carry = 0u64;
        let mut borrow = false;
        for index in 0..length {
            let product = digit as u128 * self.denominator[index] as u128 + carry as u128;
            carry = (product >> 64) as u64;
            let (first, first_borrow) = self.numerator[position + index].overflowing_sub(product as u64);
            let (second, second_borrow) = first.overflowing_sub(borrow as u64);
            self.numerator[position + index] = second;
            borrow = first_borrow || second_borrow;
        }
        let (first, first_borrow) = self.numerator[position + length].overflowing_sub(carry);
        let (second, second_borrow) = first.overflowing_sub(borrow as u64);
        self.numerator[position + length] = second;
        first_borrow || second_borrow
    }

    /// Adds the denominator back into the window at `position` (D6: the estimate was one too big).
    fn add_back(&mut self, position: usize) {
        let length = self.denominator.len();
        let mut carry = false;
        for index in 0..length {
            let (first, first_carry) = self.numerator[position + index].overflowing_add(self.denominator[index]);
            let (second, second_carry) = first.overflowing_add(carry as u64);
            self.numerator[position + index] = second;
            carry = first_carry || second_carry;
        }
        self.numerator[position + length] = self.numerator[position + length].wrapping_add(carry as u64);
    }
}
