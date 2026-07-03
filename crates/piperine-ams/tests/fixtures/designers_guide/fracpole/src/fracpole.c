/*
 * Fractional Pole
 *
 * This takes as input a start frequency, a stop frequency, a slope, the
 * unity intercept point, and the number of lumps, and synthesizes a RC
 * circuit that models a fractional impedance pole over the given frequency
 * range. The circuit is a one-port that exhibits poles and zeros that are
 * real and that are spaced evenly in a logarithmic sense over the frequency
 * range. The impedance exhibited by the one port approximates a fractional
 * pole slope between 0 and 1 in the frequency range. In other words, if
 * the impedance is plotted on a log-log scale, it will have a negative slope
 * equal to the fraction specified. The the user requested half a pole, the
 * slope will be -1/2, etc. Of course it is a lumped approximation, so the
 * slope will not be exact, but it will slowly oscillate about the desired
 * value.
 *
 * Impedance of fracpole approximates
 *     Z(s) = Coef*s^Slope
 * for 2*pi*F0 < s < 2*pi*F1.
 *
 * This model can be converted to model a fractional zero by combining it
 * with a gyrator.
 *
 * It can be used to model skin-effect loss in an inductor and dielectric
 * absorption in a capacitor, and can be used to shape white noise into
 * flicker noise.
 */

/*
 * Functions contained in this file
 *
 * fpRealize
 * fpPrint
 * fpFree
 * EvalPZ
 */

#include <stdlib.h>
#include <stdio.h>
#include <math.h>
#include "Numbers.h"
#include "Macros.h"
#include "fracpole.h"


/*
 * Default Settings
 */

#define DEFAULT_LUMPS_PER_DECADE 1



/*
 * Local Function Declarations
 */
static RealNumber EvalPZ(
    RealNumber w,
    RealVector P,
    RealVector Z,
    int Lumps,
    enum fpProfile Profile
);


/*
 * Realize Circuit that Models a Fractional Impedance Pole
 *
 * Takes the start frequency, stop frequency, the size of the circuit,
 * the slope, and a profile (describes whether the first and last roots
 * in the approximation of the impedance are zeros).
 *
 * A circuit will be synthesized that consists of a parallel connection
 * of series RC pairs, and so and admittance representation will be used.
 * A fractional pole impedance is equivalent to a fractional zero admittance.
 *
 * This function starts by generating a set of poles and zeros whose
 * admittance approximatie a fractional zero over a range of
 * frequencies. It then performs partial fraction expansion to
 * compute the residues of the poles. From the poles and residues, it
 * synthesizes an RC circuit that realizes the admittance. This process
 * is called Foster Realization and is described in
 *    Gabor C. Temes & Jack W. LaPatra
 *    Introduction to Circuit Synthesis and Design
 *    McGraw-Hill, 1977
 *
 * The output is a data structure that describes the circuit. It has
 * the following structure ...
 *
 *    o----+------+------+--...--+------+
 *         |      |      |       |      |
 *         \      \      \       \      |
 *         /      /      /  ...  /      |
 *         \ R0   \ R1   \ R2    \ Rn   |
 *         |      |      |       |      |
 *         |     ---    --- ... ---    ---
 *         |     ---    ---     ---    ---
 *         |      | C1   | C2    | Cn   | Cinf
 *    o----+------+------+--...--+------+
 *
 * Either R0 or Cinf may or may not be present (depending on the profile
 * chosen). Cinf is saved as C0. 
 *
 * If NULL is returned, there was insufficient memory to perform the
 * realization.
 */

struct fpFracPole *fpRealize(
    RealNumber Coef,	/* Unity intercept (magnitude of the impedance at w=1
			 * before approximation */
    RealNumber Slope,	/* Slope. Must be less than 0 and greater than -1. */
    RealNumber F0,	/* Lower frequency bound */
    RealNumber F1,	/* Upper frequency bound */
    RealNumber Size,	/* If Size is positive, it is interpreted as the total
			 * number of lumps needed. If the size is negative,
			 * then it is interpreted as -Lumps/Decade. If it is
			 * zero, then the default number of lumps per
			 * decade is used. */
    enum fpProfile Profile
			/* Profile specifies what happens outside the range
			 * of the approximation. It is a code that consists
			 * of a pair of letters. The first letter represents
			 * the low frequency behavior and the second represents
			 * the high frequency behavior. The letters are either
			 * F or D, F represents flat or a zero-pole slope,
			 * and D represents * down or a one-pole slope. */
)
{
    struct fpFracPole *pData;
    int i, j, n, Lumps;
    RealNumber m, m2, dm, w, wc, Kinf, norm;
    RealVector P, Z, K, G, C, tmp;

    ASSERT( F0 > 0 );
    ASSERT( F1 > F0 );
    ASSERT( Slope < 0.0 );
    ASSERT( -1.0 < Slope );

/* Determine number of lumps. */
    if (Size <= 0) {
	if (Size == 0) {
	    Size = -DEFAULT_LUMPS_PER_DECADE;
	}
	Lumps = -Size * log10( F1/F0 ) + 0.5;
    }
    else Lumps = Size + 0.5;

/* Allocate data structure and arrays. */
    pData = ALLOC( struct fpFracPole, 1 );
    G = ALLOC(RealNumber, Lumps+1);
    C = ALLOC(RealNumber, Lumps+1);
    P = ALLOC(RealNumber, Lumps+1);
    Z = ALLOC(RealNumber, Lumps+1);
    K = ALLOC(RealNumber, Lumps+1);
    if (!pData || !G || !C || !P || !Z || !K) {
	if (pData) FREE( pData );
	if (G) FREE(G);
	if (C) FREE(C);
	if (P) FREE(P);
	if (Z) FREE(Z);
	if (K) FREE(K);
	return NULL;
    }

/* Initialize output data structure. */
    pData->F0 = F0;
    pData->F1 = F1;
    pData->Coef = Coef;
    pData->Slope = Slope;
    pData->G = G;
    pData->C = C;

/* Determine frequency spacing and starting frequency of lumps. */
    if ((Profile == fpFD) || (Profile == fpDF)) {
	n = 2*Lumps - 1;
    } else {
	n = 2*Lumps;
    }
    m = pow( F1/F0, 1.0/n );
    m2 = m*m;
    dm = pow( F1/F0, Slope/n );
    w = -2.0*PI*m*F0;

/*
 * Compute critical frequencies (poles and zeros) of the admittance.
 * Poles and zeros are swapped relative to what they would be for impedance.
 */
    if (Profile & fpPOLE_AT_ZERO) {
	for (i = 1; i <= Lumps; i++) {
	    Z[i] = w * (dm * m);
	    P[i] = w / (dm * m);
	    if (i == Lumps/2)
		wc = -w; /* collocation point, used to normalize the results */
	    w *= m2;
	}
    } else {
	for (i = 1; i <= Lumps; i++) {
	    Z[i] = w * dm;
	    P[i] = w / dm;
	    if (i == Lumps/2)
		wc = -w; /* collocation point, used to normalize the results */
	    w *= m2;
	}
    }

/*
 * Determine scaling factor by evaluating ideal and lumped
 * admittance at a colocation point and and forming the ratio
 * Slope is specified for impedance, invert slope for admittance.
 */
    norm = pow(wc, -Slope) / EvalPZ(wc, P, Z, Lumps, Profile);

/*
 * Use partial fraction expansion to convert from pole-zero to pole-residue. 
 *
 * Remember the the profiles are described in terms of the impedance, but
 * the poles and zeros are of the admittance.
 */
    switch (Profile) {
        case fpFD:	/* Impedance is flat at low freqs
			 * with one-pole slope at high freqs.
			 */
	    /* Perform partial fraction expansion on admittance */
	    K[0] = norm*-Z[Lumps];
	    for (i=1; i < Lumps; i++) {
		K[0] *= -Z[i]/-P[i];
		w = P[i];
		K[i] = norm*(w-Z[i])*(w-Z[Lumps])/w;
		for (j=1; j < Lumps; j++) {
		    if (i != j)
			K[i] *= (w-Z[j])/(w-P[j]);
		}
	    }
	    Kinf = norm;
	    pData->N = Lumps-1;
            break;

        case fpDD:	/* Impedance has one-pole slope at low and high
			 * frequencies.
			 */
	    /* Perform partial fraction expansion on admittance */
	    K[0] = 0.0;
	    for (i=1; i <= Lumps; i++) {
		w = P[i];
		K[i] = norm*(w-Z[i]);
		for (j=1; j <= Lumps; j++) {
		    if (i != j)
			K[i] *= (w-Z[j])/(w-P[j]);
		}
	    }
	    Kinf = norm;
	    pData->N = Lumps;
            break;

        case fpDF:	/* Impedance has one-pole slope at low and is flat
			 * at high frequencies.
			 */
	    /* Perform partial fraction expansion on admittance */
	    K[0] = 0.0;
	    K[Lumps] = norm;
	    for (i=1; i < Lumps; i++) {
		K[Lumps] *= (P[Lumps] - Z[i])/(P[Lumps] - P[i]);
		w = P[i];
		K[i] = norm*(w-Z[i])/(w-P[Lumps]);
		for (j=1; j < Lumps; j++) {
		    if (i != j)
			K[i] *= (w-Z[j])/(w-P[j]);
		}
	    }
	    Kinf = 0.0;
	    pData->N = Lumps;
            break;

        case fpFF:	/* Impedance is flat at low and high frequencies.
			 */
	    /* Perform partial fraction expansion on admittance */
	    K[0] = norm;
	    for (i=1; i <= Lumps; i++) {
		K[0] *= -Z[i]/-P[i];
		w = P[i];
		K[i] = norm*(w-Z[i])/w;
		for (j=1; j <= Lumps; j++) {
		    if (i != j)
			K[i] *= (w-Z[j])/(w-P[j]);
		}
	    }
	    Kinf = 0.0;
	    pData->N = Lumps;
            break;

        default: ABORT();
    }

/* Convert pole-residue form to RC circuit. */
    pData->G[0] = K[0]; 
    for (i=1; i <= pData->N; i++) {
	pData->G[i] = K[i]; 
	pData->C[i] = -K[i]/P[i]; 
    }
    pData->C[0] = Kinf; 

/* Clean up */
    FREE(P);
    FREE(Z);
    FREE(K);
    return pData;
}






/*
 * Print Circuit as Spectre Subcircuit
 *
 * Takes the circuit created in fpRealize, a file pointer, and an indent
 * string.
 *
 * Prints the circuit in the form of a Spectre subcircuit to a file pointer.
 * The subcircuit is parameterized in Coef as specified to fpRealize. 
 * The indent string is prepended to every line. It is expected to contain
 * only spaces or tabs.
 */

void fpPrint(
    struct fpFracPole *pData,
    FILE *pF,
    char *SubcktName,
    char *CoefName,
    char *Indent
)
{
    int i;
    RealNumber F0 = pData->F0;
    RealNumber F1 = pData->F1;
    RealNumber Coef = pData->Coef;
    RealVector G = pData->G;
    RealVector C = pData->C;

    if (!SubcktName) SubcktName = "fracpole";
    if (!CoefName) CoefName = "coef";
    if (!Indent) Indent = "";

/* Print subckt */
    fprintf(pF,"%s// Fractional Impedance Pole\n", Indent); 
    fprintf(pF,"%s// Impedance has a slope of %lg ", Indent, pData->Slope); 
    fprintf(pF,"on a log-log scale,\n"); 
    fprintf(pF,"%s// model is valid from %0.3lg Hz to %0.3lg Hz.\n",
		Indent, F0, F1);
    fprintf(pF,"%ssubckt %s (p n)\n", Indent, SubcktName); 
    fprintf(pF,"%s    parameters %s=%lg\t", Indent, CoefName, Coef); 
    fprintf(pF,"// Impedance when angular frequency w=1\n"); 
    if (pData->G[0] != 0) {
	fprintf(pF,"%s    R0 (p n) resistor r=%lg*%s\n",
		Indent, 1/G[0], CoefName); 
    }
    for (i=1; i <= pData->N; i++) {
	fprintf(pF,"%s    R%d (p %d) resistor r=%lg*%s\n",
		Indent, i, i, 1/G[i], CoefName);
	fprintf(pF,"%s    C%d (%d n) capacitor c=%lg/%s\n",
		Indent, i, i, C[i], CoefName); 
    }
    if (pData->C[0] != 0) {
	fprintf(pF,"%s    Cinf (p n) capacitor c=%lg/%s\n",
		Indent, C[0], CoefName); 
    }
    fprintf(pF,"%sends %s\n", Indent, SubcktName); 
    return;
}





/*
 * Discard Data Structure
 *
 * Discards the circuit and frees up all memory allocated in fpRealize().
 */

void fpFree(
    struct fpFracPole *pData
)
{
    FREE(pData->G);
    FREE(pData->C);
    FREE(pData);
    return;
}





/*
 * Evalutate Pole-Zero Formulation
 *
 * Private function that takes the pole-zero representation (used in fpRealize)
 * and a frequency, and returns the magnitude of the response at that frequency.
 *
 * This is a support function for fpRealize.
 */

static RealNumber EvalPZ(
    RealNumber w,
    RealVector P,
    RealVector Z,
    int Lumps,
    enum fpProfile Profile
)
{
    RealNumber mag;
    ComplexNumber x, y;
    int i;

    switch(Profile) {
        case fpFD:
	    sclrASSIGN( y, -Z[Lumps], w);
	    for (i=1; i < Lumps; i++) {
		sclrASSIGN( x, -Z[i], w );
		cplxMULT_ASSIGN( y, x );
		sclrASSIGN( x, -P[i], w );
		cplxDIV_ASSIGN( y, x );
	    }
	    mag = cplx2_NORM(y);
	    break;

        case fpDD:
	    sclrASSIGN( y, 0.0, w);
	    for (i=1; i <= Lumps; i++) {
		sclrASSIGN( x, -Z[i], w );
		cplxMULT_ASSIGN( y, x );
		sclrASSIGN( x, -P[i], w );
		cplxDIV_ASSIGN( y, x );
	    }
	    mag = cplx2_NORM(y);
            break;

        case fpDF:
	    sclrASSIGN( y, 0.0, w);
	    sclrASSIGN( x, -P[Lumps], w);
	    cplxDIV_ASSIGN( y, x );
	    for (i=1; i < Lumps; i++) {
		sclrASSIGN( x, -Z[i], w );
		cplxMULT_ASSIGN( y, x );
		sclrASSIGN( x, -P[i], w );
		cplxDIV_ASSIGN( y, x );
	    }
	    mag = cplx2_NORM(y);
	    break;

        case fpFF:
	    sclrASSIGN( y, 1.0, 0.0);
	    for (i=1; i <= Lumps; i++) {
		sclrASSIGN( x, -Z[i], w );
		cplxMULT_ASSIGN( y, x );
		sclrASSIGN( x, -P[i], w );
		cplxDIV_ASSIGN( y, x );
	    }
	    mag = cplx2_NORM(y);
            break;

        default: ABORT();
    }
    return mag;
}
