/*
 * Definitions and declarations for FracPole code.
 */

/*
 * Impedance profiles
 */

#define fpPOLE_AT_ZERO		BIT0
#define fpZERO_AT_INFINITY	BIT1

enum fpProfile {
    /* flat at low freqs, down-slope at high freqs */
    fpFD = !fpPOLE_AT_ZERO | fpZERO_AT_INFINITY,

    /* down-slope at low freqs, down-slope at high freqs */
    fpDD = fpPOLE_AT_ZERO | fpZERO_AT_INFINITY,

    /* down-slope at low freqs, flat at high freqs */
    fpDF = fpPOLE_AT_ZERO | !fpZERO_AT_INFINITY,

    /* flat at low freqs, flat at high freqs */
    fpFF = !fpPOLE_AT_ZERO | !fpZERO_AT_INFINITY
};


/*
 * Data structures
 */
struct fpFracPole {
    RealNumber	F0;	/* User specified lower frequency bound */
    RealNumber	F1;	/* User specified upper frequency bound */
    RealNumber	Coef;	/* User specified coefficient (unity w intercept) */
    RealNumber	Slope;	/* User specified slope (on log-log plot) */
    RealVector	G;	/* Conductance array. G0, if present, is in G[0] */
    RealVector	C;	/* Capacitance array. Cinf, if present, is in C[0] */
    int		N;	/* Total number of lumps */
};


/*
 * Function declarations
 */
extern struct fpFracPole *fpRealize(
    RealNumber A,
    RealNumber Slope,
    RealNumber F0,
    RealNumber F1,
    RealNumber Size,
    enum fpProfile Profile
);
extern void fpPrint(
    struct fpFracPole *pData,
    FILE *pFile,
    char *SubcktName,
    char *CoefName,
    char *Indent
);
extern void fpFree(
    struct fpFracPole *pData
);
