#!/usr/bin/env python3
"""Reference vectors for the binary128 transcendentals of `piperine-numbers` (CV-31, T99).

Run once by hand (never by `cargo test`):  python3 generate.py

Every function is evaluated with `mpmath` (an arbitrary-precision library independent of the
Rust code) at 1024 bits with a rigorous error margin, and the result is rounded to binary128 by
EXACT rational comparison with the two neighbouring midpoints (`fractions.Fraction`); when the
margin straddles a midpoint the working precision doubles (a Ziv loop on the oracle side), so
every expected value is the correctly rounded one. Results that are exact rationals (integer
powers, `log2` of a power of two, ...) are computed with `Fraction` and never with `mpmath`.

Output, in this directory:
  <function>.tsv   rows `x[<TAB>y]<TAB>expected` (128-bit patterns as 32 hex digits; the integer
                   exponent of `powi` as a decimal), header lines start with `#`
  exact.tsv        rows `function<TAB>x<TAB>y|-<TAB>expected`: the exact cases of design-d.md
  constants.txt    pi, ln 2 and ln 10 to 1000 decimal places (mpmath)
  pi_bits.hex      pi to 20 000 bits as hex, `0x3.243F6A88...` style integer part and fraction
  half_pi.tsv      rows `k<TAB>x` (k in hex): the binary128 nearest to k*pi/2 for chosen k
"""

import datetime
import math
import os
import random
import sys
from fractions import Fraction

import mpmath
from mpmath import mp, mpf

PRECISION_BITS = 113
FRACTION_BITS = 112
MAX_EXPONENT = 16383
MIN_EXPONENT = -16382
WORKING_BITS = 1024
ROWS = 2000


class Binary128:
    """IEEE 754 binary128 patterns and exact rounding of rational numbers."""

    NAN = 0x7FFF8000000000000000000000000000
    INFINITY = 0x7FFF0000000000000000000000000000
    SIGN = 1 << 127

    @staticmethod
    def infinity(negative):
        return Binary128.INFINITY | (Binary128.SIGN if negative else 0)

    @staticmethod
    def zero(negative=False):
        return Binary128.SIGN if negative else 0

    @staticmethod
    def is_nan(bits):
        return (bits >> FRACTION_BITS) & 0x7FFF == 0x7FFF and bits & ((1 << FRACTION_BITS) - 1) != 0

    @staticmethod
    def is_infinite(bits):
        return (bits >> FRACTION_BITS) & 0x7FFF == 0x7FFF and bits & ((1 << FRACTION_BITS) - 1) == 0

    @staticmethod
    def is_negative(bits):
        return bits >> 127 == 1

    @staticmethod
    def is_zero(bits):
        return bits & ~Binary128.SIGN == 0

    @staticmethod
    def value(bits):
        """The exact value of a finite pattern."""
        field = (bits >> FRACTION_BITS) & 0x7FFF
        fraction = bits & ((1 << FRACTION_BITS) - 1)
        if field == 0:
            magnitude = Fraction(fraction) * Fraction(2) ** (MIN_EXPONENT - FRACTION_BITS)
        else:
            magnitude = Fraction(fraction | (1 << FRACTION_BITS)) * Fraction(2) ** (field - 16383 - FRACTION_BITS)
        return -magnitude if bits >> 127 else magnitude

    @staticmethod
    def round(value, negative_zero=False):
        """The pattern nearest to the exact rational `value`, ties to even, overflow to infinity."""
        if value == 0:
            return Binary128.zero(negative_zero)
        negative = value < 0
        magnitude = -value if negative else value
        exponent = magnitude.numerator.bit_length() - magnitude.denominator.bit_length()
        if Fraction(2) ** exponent > magnitude:
            exponent -= 1
        unit = max(exponent, MIN_EXPONENT) - FRACTION_BITS
        scaled = magnitude / Fraction(2) ** unit
        quotient = scaled.numerator // scaled.denominator
        remainder = scaled - quotient
        if remainder > Fraction(1, 2) or (remainder == Fraction(1, 2) and quotient & 1):
            quotient += 1
        if quotient == 1 << PRECISION_BITS:
            quotient >>= 1
            unit += 1
        sign = Binary128.SIGN if negative else 0
        if quotient < 1 << FRACTION_BITS:
            return sign | quotient
        field = unit + FRACTION_BITS + 16383
        if field >= 0x7FFF:
            return sign | Binary128.INFINITY
        return sign | field << FRACTION_BITS | (quotient - (1 << FRACTION_BITS))


class Oracle:
    """High-precision evaluation rounded exactly to binary128."""

    @staticmethod
    def to_mpf(value):
        return mpf(value.numerator) / mpf(value.denominator)

    @staticmethod
    def to_fraction(number):
        sign, mantissa, exponent, _ = number._mpf_
        if mantissa == 0:
            return Fraction(0)
        magnitude = Fraction(mantissa) * (Fraction(2) ** exponent)
        return -magnitude if sign else magnitude

    @staticmethod
    def pi_over_two(bits):
        with mp.workprec(bits):
            return Oracle.to_fraction(mp.pi / 2)

    @staticmethod
    def round(evaluate, *arguments, zero_negative=False):
        """`evaluate` receives mpf arguments and returns an mpf; the result is the correctly rounded pattern."""
        precision = WORKING_BITS
        while precision <= 1 << 16:
            with mp.workprec(precision):
                exact_arguments = [Oracle.to_mpf(argument) for argument in arguments]
                result = evaluate(*exact_arguments)
                sign, mantissa, exponent, bit_count = result._mpf_
                if mantissa and exponent + bit_count > 17000:
                    return Binary128.infinity(sign == 1)
                if mantissa and exponent + bit_count < -17000:
                    return Binary128.zero(sign == 1)
                fraction = Oracle.to_fraction(result)
                margin = abs(fraction) * Fraction(1, 2 ** (precision - 40)) + Fraction(1, 2 ** (precision + 20000))
            low = Binary128.round(fraction - margin, zero_negative)
            high = Binary128.round(fraction + margin, zero_negative)
            if low == high:
                return low
            precision *= 2
        raise RuntimeError("undecided rounding: a midpoint? " + repr(arguments))


class Inputs:
    """Deterministic input sets."""

    def __init__(self, seed):
        self.random = random.Random(seed)

    def bits(self):
        """A random finite pattern."""
        while True:
            pattern = self.random.getrandbits(128)
            if (pattern >> FRACTION_BITS) & 0x7FFF != 0x7FFF:
                return pattern

    def value(self, low, high, positive=False):
        """A random finite pattern with unbiased exponent in [low, high]."""
        exponent = self.random.randint(low, high)
        fraction = self.random.getrandbits(FRACTION_BITS)
        if self.random.random() < 0.2:
            fraction &= ~((1 << self.random.randint(0, FRACTION_BITS)) - 1)
        sign = 0 if positive else self.random.getrandbits(1) << 127
        if exponent < MIN_EXPONENT:
            shifted = ((fraction | 1 << FRACTION_BITS) >> (MIN_EXPONENT - exponent)) or 1
            return sign | shifted
        return sign | (exponent + 16383) << FRACTION_BITS | fraction

    def fraction(self, low, high):
        """A random pattern for a real number uniform in [low, high]."""
        denominator = 1 << 200
        numerator = self.random.randint(0, denominator)
        return Binary128.round(Fraction(low) + (Fraction(high) - Fraction(low)) * Fraction(numerator, denominator))

    def half_pi_multiples(self, count):
        """Patterns nearest to k * pi / 2 for odd k of random size (worst cases of argument reduction)."""
        half_pi = Oracle.pi_over_two(17500)
        patterns = []
        for _ in range(count):
            bits = self.random.randint(1, 16370)
            k = self.random.getrandbits(bits) | 1
            patterns.append(Binary128.round(k * half_pi))
        return patterns

    def specials(self):
        """Zeros, infinities, NaN, extremes and simple values of both signs."""
        positive = [0, 1, 2, (1 << FRACTION_BITS) - 1, 1 << FRACTION_BITS, (1 << FRACTION_BITS) + 1,
                    Binary128.round(Fraction(1)), Binary128.round(Fraction(1, 2)), Binary128.round(Fraction(2)),
                    Binary128.round(Fraction(3, 2)), Binary128.round(Fraction(3)), Binary128.round(Fraction(10)),
                    0x7FFEFFFFFFFFFFFFFFFFFFFFFFFFFFFF, 0x7FFE0000000000000000000000000000,
                    Binary128.INFINITY, Binary128.NAN]
        return positive + [pattern | Binary128.SIGN for pattern in positive if pattern != Binary128.NAN]

    def around(self, number, count, relative_exponent=-110):
        """Patterns near the real `number`: it, its neighbours and random small offsets."""
        base = Binary128.round(Fraction(number))
        patterns = [pattern for pattern in (base, base + 1, base - 1) if 0 <= pattern < 1 << 128 and pattern >> 127 == base >> 127]
        for _ in range(count):
            offset = Fraction(self.random.randint(-1000, 1000), 1000) * Fraction(2) ** self.random.randint(relative_exponent - 30, relative_exponent + 30)
            patterns.append(Binary128.round(Fraction(number) * (1 + offset)))
        return patterns


class Function:
    """One transcendental: how to evaluate it, its special values, its input rows."""

    def __init__(self, name, arity, evaluate, special, inputs):
        self.name = name
        self.arity = arity
        self.evaluate = evaluate
        self.special = special
        self.inputs = inputs

    def expected(self, *patterns):
        special = self.special(*patterns)
        if special is not None:
            return special
        return Oracle.round(self.evaluate, *[Binary128.value(pattern) for pattern in patterns])


class Rules:
    """Special-value rules of IEEE 754-2019 section 9.2 as Rust's methods give them."""

    @staticmethod
    def exp(x):
        if Binary128.is_nan(x):
            return Binary128.NAN
        if Binary128.is_infinite(x):
            return Binary128.zero() if Binary128.is_negative(x) else Binary128.INFINITY
        if Binary128.is_zero(x):
            return Binary128.round(Fraction(1))
        value = Binary128.value(x)
        if value > 20000:
            return Binary128.INFINITY
        if value < -20000:
            return Binary128.zero()
        return None

    @staticmethod
    def logarithm(exact):
        """`exact` maps a positive finite value to an exact pattern or None."""
        def rule(x):
            if Binary128.is_nan(x):
                return Binary128.NAN
            if Binary128.is_zero(x):
                return Binary128.infinity(True)
            if Binary128.is_negative(x):
                return Binary128.NAN
            if Binary128.is_infinite(x):
                return Binary128.INFINITY
            return exact(Binary128.value(x))
        return rule

    @staticmethod
    def exact_ln(value):
        return Binary128.zero() if value == 1 else None

    @staticmethod
    def exact_log2(value):
        numerator, denominator = value.numerator, value.denominator
        if numerator == 1 and denominator & (denominator - 1) == 0:
            return Binary128.round(Fraction(-(denominator.bit_length() - 1)))
        if denominator == 1 and numerator & (numerator - 1) == 0:
            return Binary128.round(Fraction(numerator.bit_length() - 1))
        return None

    @staticmethod
    def exact_log10(value):
        if value.denominator == 1:
            for power in range(0, 49):
                if value == 10 ** power:
                    return Binary128.round(Fraction(power))
        return None

    @staticmethod
    def sine(x):
        if Binary128.is_nan(x) or Binary128.is_infinite(x):
            return Binary128.NAN
        if Binary128.is_zero(x):
            return x
        return None

    @staticmethod
    def cosine(x):
        if Binary128.is_nan(x) or Binary128.is_infinite(x):
            return Binary128.NAN
        if Binary128.is_zero(x):
            return Binary128.round(Fraction(1))
        return None

    @staticmethod
    def arcsine(x):
        if Binary128.is_nan(x) or Binary128.is_infinite(x):
            return Binary128.NAN
        if Binary128.is_zero(x):
            return x
        if abs(Binary128.value(x)) > 1:
            return Binary128.NAN
        return None

    @staticmethod
    def arccosine(x):
        if Binary128.is_nan(x) or Binary128.is_infinite(x):
            return Binary128.NAN
        value = Binary128.value(x)
        if abs(value) > 1:
            return Binary128.NAN
        if value == 1:
            return Binary128.zero()
        return None

    @staticmethod
    def arctangent(x):
        if Binary128.is_nan(x):
            return Binary128.NAN
        if Binary128.is_zero(x):
            return x
        if Binary128.is_infinite(x):
            with mp.workprec(WORKING_BITS):
                half_pi = Oracle.to_fraction(mp.pi / 2)
            return Binary128.round(-half_pi if Binary128.is_negative(x) else half_pi)
        return None

    @staticmethod
    def hyperbolic_sine(x):
        if Binary128.is_nan(x):
            return Binary128.NAN
        if Binary128.is_infinite(x) or Binary128.is_zero(x):
            return x
        value = Binary128.value(x)
        if abs(value) > 20000:
            return Binary128.infinity(value < 0)
        return None

    @staticmethod
    def hyperbolic_cosine(x):
        if Binary128.is_nan(x):
            return Binary128.NAN
        if Binary128.is_infinite(x):
            return Binary128.INFINITY
        if Binary128.is_zero(x):
            return Binary128.round(Fraction(1))
        if abs(Binary128.value(x)) > 20000:
            return Binary128.INFINITY
        return None

    @staticmethod
    def hyperbolic_tangent(x):
        if Binary128.is_nan(x):
            return Binary128.NAN
        if Binary128.is_zero(x):
            return x
        if Binary128.is_infinite(x) or abs(Binary128.value(x)) > 20000:
            return Binary128.round(Fraction(-1 if Binary128.is_negative(x) else 1))
        return None

    @staticmethod
    def is_integer(value):
        return value.denominator == 1

    @staticmethod
    def is_odd_integer(value):
        return value.denominator == 1 and value.numerator & 1 == 1

    @staticmethod
    def power(x, y):
        """`powf`: the exact special cases of C's `pow`, then exact rational powers, else None."""
        if Binary128.is_zero(y):
            return Binary128.round(Fraction(1))
        if Binary128.value(x) == 1 if not (Binary128.is_nan(x) or Binary128.is_infinite(x)) else False:
            return Binary128.round(Fraction(1))
        if Binary128.is_nan(x) or Binary128.is_nan(y):
            return Binary128.NAN
        y_value = None if Binary128.is_infinite(y) else Binary128.value(y)
        x_negative = Binary128.is_negative(x)
        if Binary128.is_zero(x):
            odd = y_value is not None and Rules.is_odd_integer(y_value)
            if Binary128.is_negative(y):
                return Binary128.infinity(x_negative and odd)
            return Binary128.zero(x_negative and odd)
        if Binary128.is_infinite(x):
            odd = y_value is not None and Rules.is_odd_integer(y_value)
            if Binary128.is_negative(y):
                return Binary128.zero(x_negative and odd)
            return Binary128.infinity(x_negative and odd)
        x_value = Binary128.value(x)
        if Binary128.is_infinite(y):
            magnitude = abs(x_value)
            if magnitude == 1:
                return Binary128.round(Fraction(1))
            grows = (magnitude > 1) == (not Binary128.is_negative(y))
            return Binary128.INFINITY if grows else Binary128.zero()
        if x_negative and not Rules.is_integer(y_value):
            return Binary128.NAN
        exact = Rules.exact_power(abs(x_value), y_value)
        negative = x_negative and Rules.is_odd_integer(y_value)
        if exact == "overflow":
            return Binary128.infinity(negative)
        if exact == "underflow":
            return Binary128.zero(negative)
        if exact is not None:
            return Binary128.round(-exact if negative else exact, negative)
        return None

    @staticmethod
    def exact_power(base, exponent):
        """`base ** exponent` as an exact Fraction when it is rational and small, a range verdict
        for a hopeless magnitude, else None (the oracle evaluates it)."""
        if base == 1:
            return Fraction(1)
        top = base.numerator.bit_length() - base.denominator.bit_length()
        if abs(top) * abs(exponent) > 40000:
            direction = (top > 0) == (exponent > 0)
            return "overflow" if direction else "underflow"
        if exponent.denominator == 1 and abs(exponent) <= 200:
            return base ** int(exponent)
        if exponent.denominator == 2 and abs(exponent) <= 200:
            numerator_root = Rules.integer_root(base.numerator)
            denominator_root = Rules.integer_root(base.denominator)
            if numerator_root is not None and denominator_root is not None:
                return Fraction(numerator_root, denominator_root) ** int(exponent * 2)
        return None

    @staticmethod
    def integer_root(number):
        root = math.isqrt(number)
        return root if root * root == number else None

    @staticmethod
    def power_integer(x, exponent):
        """`powi`: x ** exponent for an i32 exponent."""
        if exponent == 0:
            return Binary128.round(Fraction(1))
        if Binary128.is_nan(x):
            return Binary128.NAN
        odd = exponent & 1 == 1
        negative = Binary128.is_negative(x) and odd
        if Binary128.is_zero(x):
            return Binary128.infinity(negative) if exponent < 0 else Binary128.zero(negative)
        if Binary128.is_infinite(x):
            return Binary128.zero(negative) if exponent < 0 else Binary128.infinity(negative)
        value = Binary128.value(x)
        exact = Rules.exact_power(abs(value), Fraction(exponent))
        if exact == "overflow":
            return Binary128.infinity(negative)
        if exact == "underflow":
            return Binary128.zero(negative)
        if exact is not None:
            return Binary128.round(-exact if negative else exact, negative)
        return None

    @staticmethod
    def arctangent_two(y, x):
        """`y.atan2(x)` specials; None for finite nonzero arguments."""
        if Binary128.is_nan(y) or Binary128.is_nan(x):
            return Binary128.NAN
        with mp.workprec(WORKING_BITS):
            pi = Oracle.to_fraction(mp.pi)
        y_negative = Binary128.is_negative(y)
        sign = -1 if y_negative else 1
        x_negative = Binary128.is_negative(x)
        if Binary128.is_zero(y):
            if Binary128.is_zero(x):
                return Binary128.round(Fraction(sign) * (pi if x_negative else 0), y_negative)
            return Binary128.round(Fraction(sign) * (pi if x_negative else 0), y_negative)
        if Binary128.is_infinite(y):
            if Binary128.is_infinite(x):
                return Binary128.round(Fraction(sign) * (3 * pi / 4 if x_negative else pi / 4))
            return Binary128.round(Fraction(sign) * pi / 2)
        if Binary128.is_infinite(x):
            return Binary128.round(Fraction(sign) * (pi if x_negative else 0), y_negative)
        if Binary128.is_zero(x):
            return Binary128.round(Fraction(sign) * pi / 2)
        return None


class Generator:
    """Builds the sets and writes the files."""

    def __init__(self):
        self.directory = os.path.dirname(os.path.abspath(__file__))
        self.stamp = "generator: generate.py; mpmath %s; %s" % (mpmath.__version__, datetime.date.today().isoformat())

    def unique(self, rows):
        seen = set()
        result = []
        for row in rows:
            if row not in seen:
                seen.add(row)
                result.append(row)
        return result

    def unary_inputs(self, source, spec):
        """`spec` lists (kind, count, args); kind is bits, value, fraction, around or the specials."""
        patterns = list(source.specials())
        for kind, count, *args in spec:
            for _ in range(count):
                if kind == "bits":
                    patterns.append(source.bits())
                elif kind == "value":
                    patterns.append(source.value(*args))
                elif kind == "fraction":
                    patterns.append(source.fraction(*args))
            if kind == "around":
                patterns.extend(source.around(args[0], count))
            if kind == "half_pi":
                patterns.extend(source.half_pi_multiples(count))
        return self.unique(patterns)

    def write_unary(self, function, patterns, rows):
        path = "%s/%s.tsv" % (self.directory, function.name)
        with open(path, "w") as out:
            out.write("# %s\n# columns: x (binary128 bits, hex) <TAB> expected (binary128 bits, hex)\n" % self.stamp)
            for pattern in patterns:
                expected = function.expected(pattern)
                out.write("%032x\t%032x\n" % (pattern, expected))
                rows.append((function.name, pattern, None, expected))
        print(function.name, len(patterns))

    def integer_power(self, x, exponent):
        special = Rules.power_integer(x, exponent)
        if special is not None:
            return special
        return Oracle.round(lambda base: mp.power(base, exponent), Binary128.value(x))

    def write_binary(self, function, pairs, rows, second_is_integer=False):
        path = "%s/%s.tsv" % (self.directory, function.name)
        with open(path, "w") as out:
            kind = "e (i32, decimal)" if second_is_integer else "y (binary128 bits, hex)"
            out.write("# %s\n# columns: x (binary128 bits, hex) <TAB> %s <TAB> expected (binary128 bits, hex)\n" % (self.stamp, kind))
            for first, second in pairs:
                if second_is_integer:
                    expected = self.integer_power(first, second)
                    out.write("%032x\t%d\t%032x\n" % (first, second, expected))
                else:
                    expected = function.expected(first, second)
                    out.write("%032x\t%032x\t%032x\n" % (first, second, expected))
                rows.append((function.name, first, second, expected))
        print(function.name, len(pairs))


class Exact:
    """The exact cases, constants and reduction data files."""

    def __init__(self, generator):
        self.generator = generator

    def rows(self, source):
        one = Binary128.round(Fraction(1))
        rows = []
        zeros = [0, Binary128.SIGN]
        for z in zeros:
            rows.append(("exp", z, None, one))
            rows.append(("cos", z, None, one))
            rows.append(("cosh", z, None, one))
            for name in ["sin", "tan", "asin", "atan", "sinh", "tanh"]:
                rows.append((name, z, None, z))
        rows.append(("ln", one, None, 0))
        rows.append(("log2", one, None, 0))
        rows.append(("log10", one, None, 0))
        rows.append(("acos", one, None, 0))
        for k in list(range(-16494, 16384, 61)) + [-16494, -16382, -1, 0, 1, 2, 16383]:
            x = Binary128.round(Fraction(2) ** k)
            rows.append(("log2", x, None, Rules.exact_log2(Binary128.value(x))))
        for k in range(0, 49):
            x = Binary128.round(Fraction(10) ** k)
            rows.append(("log10", x, None, Rules.exact_log10(Binary128.value(x))))
        specials = source.specials()
        for y in specials:
            for x in specials:
                if Binary128.is_zero(y) or Binary128.is_infinite(y) or Binary128.is_zero(x) or Binary128.is_infinite(x):
                    expected = Rules.arctangent_two(y, x)
                    if expected is not None:
                        rows.append(("atan2", y, x, expected))
        for _ in range(60):
            root = source.random.randint(1 << 37, (1 << 38) - 1) | 1
            x = Binary128.round(Fraction(root * root))
            y = Binary128.round(Fraction(3, 2))
            rows.append(("powf", x, y, Rules.power(x, y)))
        for _ in range(60):
            base = source.random.randint(3, 1 << 20) | 1
            exponent = source.random.randint(2, 5)
            x, y = Binary128.round(Fraction(base)), Binary128.round(Fraction(exponent))
            rows.append(("powf", x, y, Rules.power(x, y)))
            rows.append(("powf", x | Binary128.SIGN, y, Rules.power(x | Binary128.SIGN, y)))
            rows.append(("powi", x, exponent, Rules.power_integer(x, exponent)))
            rows.append(("powi", x, -exponent, Rules.power_integer(x, -exponent)))
        for k in range(-30, 31, 3):
            x = Binary128.round(Fraction(2) ** k)
            rows.append(("powi", x, 7, Rules.power_integer(x, 7)))
            rows.append(("powf", x, Binary128.round(Fraction(1, 2)), Rules.power(x, Binary128.round(Fraction(1, 2)))))
        return [row for row in rows if row[3] is not None]

    def write(self, source):
        path = "%s/exact.tsv" % self.generator.directory
        rows = self.rows(source)
        with open(path, "w") as out:
            out.write("# %s\n# columns: function <TAB> x (hex) <TAB> y (hex, or decimal for powi, or -) <TAB> expected (hex); every row an exact case computed with Fraction\n" % self.generator.stamp)
            for name, x, y, expected in rows:
                second = "-" if y is None else ("%d" % y if name == "powi" else "%032x" % y)
                out.write("%s\t%032x\t%s\t%032x\n" % (name, x, second, expected))
        print("exact", len(rows))

    def write_constants(self):
        with mp.workdps(1100):
            values = [("pi", str(+mp.pi)), ("ln2", str(mp.log(2))), ("ln10", str(mp.log(10)))]
        published = {
            "pi": "3.14159265358979323846264338327950288419716939937510582097494459230781640628620899",
            "ln2": "0.69314718055994530941723212145817656807550013436025525412068000949339362196969471",
            "ln10": "2.30258509299404568401799145468436420760110148862877297603332790096757260967735248",
        }
        with open("%s/constants.txt" % self.generator.directory, "w") as out:
            out.write("# %s\n# name <TAB> value truncated to 1000 decimal places\n" % self.generator.stamp)
            for name, text in values:
                assert text.startswith(published[name]), name
                whole, fraction = text.split(".")
                out.write("%s\t%s.%s\n" % (name, whole, fraction[:1000]))
        with mp.workprec(20200):
            scaled = int(mp.floor(mp.pi * mpf(2) ** 20000))
        with open("%s/pi_bits.hex" % self.generator.directory, "w") as out:
            out.write("# %s\n# floor(pi * 2^20000) in hex\n%x\n" % (self.generator.stamp, scaled))

    def write_half_pi(self, source):
        half_pi = Oracle.pi_over_two(17500)
        path = "%s/half_pi.tsv" % self.generator.directory
        with open(path, "w") as out:
            out.write("# %s\n# columns: k (hex, odd) <TAB> binary128 nearest to k * pi / 2 (hex)\n" % self.generator.stamp)
            for _ in range(300):
                bits = source.random.randint(1, 16370)
                k = source.random.getrandbits(bits) | 1
                out.write("%x\t%032x\n" % (k, Binary128.round(k * half_pi)))
        print("half_pi 300")


class Main:
    """Entry point."""

    @staticmethod
    def run():
        generator = Generator()
        rows = []
        source = Inputs(20261123)
        s = source

        def unary(name, evaluate, special, spec):
            function = Function(name, 1, evaluate, special, None)
            generator.write_unary(function, generator.unary_inputs(source, spec), rows)

        unary("exp", mp.exp, Rules.exp, [("value", 700, -120, 13), ("fraction", 500, -11500, 11500), ("around", 150, 11356.5), ("around", 150, -11356.5), ("around", 150, -11433.5), ("around", 100, 11433.0), ("value", 150, -16494, -100), ("bits", 100)])
        unary("ln", mp.log, Rules.logarithm(Rules.exact_ln), [("value", 600, -16494, 16383, True), ("fraction", 500, 0, 4), ("around", 250, 1), ("around", 150, 2), ("around", 150, 0.5), ("value", 200, -16494, -16382, True), ("value", 100, 16300, 16383, True)])
        unary("log2", lambda x: mp.log(x) / mp.log(2), Rules.logarithm(Rules.exact_log2), [("value", 600, -16494, 16383, True), ("fraction", 400, 0, 100), ("around", 200, 1), ("around", 150, 2), ("around", 150, 4), ("value", 200, -16494, -16382, True)])
        unary("log10", lambda x: mp.log10(x), Rules.logarithm(Rules.exact_log10), [("value", 600, -16494, 16383, True), ("fraction", 400, 0, 1000), ("around", 200, 1), ("around", 150, 10), ("around", 150, 100), ("value", 200, -16494, -16382, True)])
        unary("sin", mp.sin, Rules.sine, [("half_pi", 300), ("value", 600, -8, 10), ("bits", 150), ("value", 150, 100, 16383), ("value", 250, -16494, -100), ("fraction", 300, -7, 7)])
        unary("cos", mp.cos, Rules.cosine, [("half_pi", 300), ("value", 600, -8, 10), ("bits", 150), ("value", 150, 100, 16383), ("value", 250, -16494, -100), ("fraction", 300, -7, 7)])
        unary("tan", mp.tan, Rules.sine, [("half_pi", 300), ("value", 600, -8, 10), ("bits", 150), ("value", 150, 100, 16383), ("value", 250, -16494, -100), ("fraction", 300, -7, 7)])
        unary("asin", mp.asin, Rules.arcsine, [("fraction", 700, -1, 1), ("around", 200, 1), ("around", 200, -1), ("value", 300, -16494, -1), ("value", 100, 0, 30)])
        unary("acos", mp.acos, Rules.arccosine, [("fraction", 700, -1, 1), ("around", 200, 1), ("around", 200, -1), ("value", 300, -16494, -1), ("value", 100, 0, 30)])
        unary("atan", mp.atan, Rules.arctangent, [("value", 700, -30, 30), ("bits", 300), ("value", 200, 100, 16383), ("value", 200, -16494, -100), ("fraction", 200, -3, 3)])
        unary("sinh", mp.sinh, Rules.hyperbolic_sine, [("value", 800, -130, 8), ("fraction", 300, -60, 60), ("around", 100, 11356.6), ("around", 100, -11356.6), ("value", 250, -16494, -130), ("bits", 100)])
        unary("cosh", mp.cosh, Rules.hyperbolic_cosine, [("value", 800, -130, 8), ("fraction", 300, -60, 60), ("around", 100, 11356.6), ("around", 100, -11356.6), ("value", 250, -16494, -130), ("bits", 100)])
        unary("tanh", mp.tanh, Rules.hyperbolic_tangent, [("value", 800, -130, 8), ("fraction", 300, -60, 60), ("around", 100, 40), ("around", 100, -40), ("value", 250, -16494, -130), ("bits", 100)])

        # powf: pairs
        power = Function("powf", 2, lambda x, y: mp.power(x, y), Rules.power, None)
        pairs = [(a, b) for a in source.specials() for b in source.specials()]
        for _ in range(500):
            pairs.append((source.value(-30, 30, True), source.value(-10, 10)))
        for _ in range(250):
            pairs.append((source.value(-16494, 16383, True), source.value(-12, 3)))
        for _ in range(250):
            pairs.append((source.value(-20, 20), Binary128.round(Fraction(source.random.randint(-30, 30)))))
        for _ in range(150):
            pairs.append((source.value(-5, 5, True), Binary128.round(Fraction(source.random.randint(-60, 60), 2))))
        for _ in range(100):
            root = source.random.randint(1 << 37, (1 << 38) - 1) | 1
            pairs.append((Binary128.round(Fraction(root * root)), Binary128.round(Fraction(3, 2))))
        for _ in range(100):
            root = source.random.randint(1 << 20, 1 << 27) | 1
            pairs.append((Binary128.round(Fraction(root)), Binary128.round(Fraction(source.random.randint(2, 4)))))
        for _ in range(250):
            pairs.append((source.value(0, 40, True), source.value(5, 16)))
        for _ in range(100):
            pairs.append((source.value(-20, 20, True), source.value(-16494, -50)))
        generator.write_binary(power, generator.unique(pairs), rows)

        # powi: x and an i32 exponent
        powi = Function("powi", 2, None, None, None)
        pairs = [(a, e) for a in source.specials() for e in (0, 1, -1, 2, -2, 3, 2 ** 31 - 1, -2 ** 31)]
        for _ in range(800):
            pairs.append((source.value(-40, 40), source.random.randint(-300, 300)))
        for _ in range(300):
            pairs.append((source.value(-3, 3), source.random.randint(-2 ** 31, 2 ** 31 - 1)))
        for _ in range(300):
            pairs.append((Binary128.round(1 + Fraction(1, 1 << source.random.randint(20, 112))), source.random.randint(-2 ** 31, 2 ** 31 - 1)))
        for _ in range(200):
            pairs.append((source.value(-16494, 16383), source.random.randint(-6, 6)))
        generator.write_binary(powi, generator.unique(pairs), rows, second_is_integer=True)

        # atan2: pairs (y, x)
        atan2 = Function("atan2", 2, lambda y, x: mp.atan2(y, x), Rules.arctangent_two, None)
        pairs = [(a, b) for a in source.specials() for b in source.specials()]
        for _ in range(900):
            pairs.append((source.value(-5, 5), source.value(-5, 5)))
        for _ in range(400):
            pairs.append((source.value(-40, 40), source.value(-40, 40)))
        for _ in range(300):
            pairs.append((source.value(-16494, 16383), source.value(-16494, 16383)))
        for _ in range(200):
            pairs.append((source.value(-3, 3), Binary128.round(Fraction(source.random.randint(-3, 3)))))
        generator.write_binary(atan2, generator.unique(pairs), rows)

        exact = Exact(generator)
        exact.write(source)
        exact.write_constants()
        exact.write_half_pi(source)


if __name__ == "__main__":
    Main.run()
