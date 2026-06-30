/*
 * Need to compute F0 and F1 from Rleak, ESR, and Zc (a non-trivial exercise).
 */
/*
 * Generates an approximate model for a capacitor exibiting dieletric absorption
 */

#define PI 3.14159265358979323846264338327950288419716939937511
#define VERSION "1.0 (10 Oct. 2001)"

#include <stdlib.h>
#include <stdio.h>
#include <math.h>
#include <errno.h>
#include "Macros.h"
#include "Numbers.h"
#include "fracpole.h"

main(argc, argv)
int argc;
char *argv[];
{
double F1=0, F0=0, C0=0, Cinf=0, Tau0=0, Alpha=0;
double ESR=0, ESL=0, Rleak=0;
char *myname = argv[0], *profile = "fd", *subcktName = "", fileName[BUFSIZ];
int i;
double size=0, Coef;
struct fpFracPole *pFP;
enum fpProfile profileCode;
FILE *pF;
extern int errno;
extern char *strerror();

/* read command line arguments */
    for (i = 1; i < argc; i++) {
	if (!strncmp(argv[i], "f0=",3))
	    F0 = atof(argv[i]+3);
	else if (!strncmp(argv[i], "f1=", 3))
	    F1 = atof(argv[i]+3);
	else if (!strncmp(argv[i], "cinf=", 5))
	    Cinf = atof(argv[i]+5);
	else if (!strncmp(argv[i], "c0=", 3))
	    C0 = atof(argv[i]+3);
	else if (!strncmp(argv[i], "tau0=", 5))
	    Tau0 = atof(argv[i]+5);
	else if (!strncmp(argv[i], "alpha=", 6))
	    Alpha = atof(argv[i]+6);
	else if (!strncmp(argv[i], "esr=", 4))
	    ESR = atof(argv[i]+4);
	else if (!strncmp(argv[i], "esl=", 4))
	    ESL = atof(argv[i]+4);
	else if (!strncmp(argv[i], "rleak=", 6))
	    Rleak = atof(argv[i]+6);
	else if (!strncmp(argv[i], "lumps=", 6))
	    size = atof(argv[i]+6);
	else if (!strncmp(argv[i], "profile=", 8))
	    profile = argv[i]+8;
	else if (!strncmp(argv[i], "name=", 5))
	    subcktName = argv[i]+5;
	else if (!strncmp(argv[i], "-V", 2) || !strncmp(argv[i], "-v", 2)) {
	    printf("%s\n", VERSION);
	    exit(0);
	}
	else {
	    if (strncmp(argv[i], "-h",2))
		fprintf(stderr,"%s: unknown argument: %s\n\n", myname, argv[i]);
	    helpMessage(myname);
	    exit(0);
	}
    }

/* Assure all parameters were given. */
    if (C0 <= 0 || Cinf <= 0 || Tau0 <= 0 || Alpha <= 0 ||
        subcktName[0] == '\0')
    {
        fprintf(stderr, "Error: Parameters name, cinf, c0, tau0, and alpha ");
        fprintf(stderr, "must all be given and greater than 0.\n");
        fprintf(stderr, "Example: %s name=lossycap cinf=10e-9 ", myname);
        fprintf(stderr, "c0=22.5e-9 tau0=1 alpha=.75 f0=0.01 f1=1e5\n");
        exit(1);
    }
    if (F0 <= 0) {
	fprintf(stderr, "Error: f0 must be given and greater than 0.\n");
	fprintf(stderr, "    Use \"%s -h\" for more information\n", myname);
	exit(1);
    }
    if (F1 <= F0) {
	fprintf(stderr, "Error: f1 must be given and greater than f0.\n");
	fprintf(stderr, "    Use \"%s -h\" for more information\n", myname);
	exit(1);
    }
    if (C0 <= Cinf) {
	fprintf(stderr, "Error: c0 must be greater than cinf.\n");
	exit(1);
    }
    if (1 <= Alpha) {
	fprintf(stderr, "Error: alpha must be between 0 and 1.\n");
	exit(1);
    }
    if (ESR < 0) {
	fprintf(stderr, "Error: esr must be greater than 0.\n");
	exit(1);
    }
    if (ESL < 0) {
	fprintf(stderr, "Error: esl must be greater than 0.\n");
	exit(1);
    }

/* Validate user specified profile and convert it to enum. */
    if (!strcmp(profile, "fd"))
	profileCode = fpFD;
    else if (!strcmp(profile, "dd"))
	profileCode = fpDD;
    else if (!strcmp(profile, "df"))
	profileCode = fpDF;
    else if (!strcmp(profile, "ff"))
	profileCode = fpFF;
    else {
	fprintf(stderr, "`%s' not valid value for `profile', ", profile);
	fprintf(stderr, "use either `fd', `dd', `df', or `ff'.");
	exit(1);
    }

/* Calculate coefficient for fractional impedance pole */
    Coef = pow(Tau0, 1-Alpha) / (C0 - Cinf);

/* Realize dielectric absorption subcircuit */
    pFP = fpRealize( Coef, -Alpha, F0, F1, size, profileCode);
    if (pFP == NULL) {
	fprintf( stderr, "Error: insufficient memory available.");
	exit(1);
    }

/* Output realized circuit to a file in the form of a Spectre subcircuit */
    sprintf(fileName, "lossycap.scs");
    pF = fopen(fileName, "w");
    if (pF == NULL) {
	fprintf( stderr, "Error: cannot write to file `lossycap.scs'\n   %s.\n",
		 strerror( errno ));
	exit(1);
    }

/* Generate subcircuit of absorptive capacitor */
    fprintf(pF, "simulator lang=spectre\n\n");
    fprintf(pF, "//\n" );
    fprintf(pF, "// Capacitor model that include dielectric absorption\n" );
    fprintf(pF, "//\n" );
    fprintf(pF, "subckt %s (1 2)\n", subcktName);
    if ((ESR != 0) || (ESL != 0)) {
	if (ESL != 0)
	    fprintf(pF, "    L (1 3) inductor l=%lg r=%lg\n", ESL, ESR);
	else
	    fprintf(pF, "    R  (1 3) resistor r=%lg\n", ESR);
	fprintf(pF, "    C  (3 2) capacitor c=%lg\n", Cinf);
	if (Rleak != 0)
	    fprintf(pF, "    Rl (3 2) resistor r=%lg\n", Rleak);
	fprintf(pF, "    Cx (3 4) capacitor c=%lg\n", C0 - Cinf);
	fprintf(pF, "    DA (4 2) fracpole\n\n");
    } else {
	fprintf(pF, "    C  (1 2) capacitor c=%lg\n", Cinf);
	if (Rleak != 0)
	    fprintf(pF, "    Rl (1 2) resistor r=%lg\n", Rleak);
	fprintf(pF, "    Cx (1 3) capacitor c=%lg\n", C0 - Cinf);
	fprintf(pF, "    DA (3 2) fracpole\n\n");
    }

/* Print absorption subcircuit */
    fpPrint(pFP, pF, "fracpole", NULL, "    ");
    fpFree(pFP);

/* Terminate open subcircuit definitions and close file. */
    fprintf(pF, "ends %s\n", subcktName);
    fclose(pF);

    exit(0);
}



helpMessage(myname)
char *myname;
{
char *intro = "\
%s:\
    Generates a Spectre subcircuit description of a capacitor that exhibits\n\
    dielectric absorption.\n";

char *usage = "\
Usage:\n\
%s name=<subcktname> cinf=<val> c0=<val> tau0=<val> alpha=<val> \\\n\
		    f0=<val> [f1=<val>] [esr=<val>] [esl=<val>] \\\n\
		    [rleak=<val>] [lumps=<val>] [profile=<fd, dd, df or ff>]\n\
where\n\
    f0 is the low frequency limit for dielectric absorption model in hertz\n\
    f1 is the high frequency limit for dielectric absorption model in hertz\n\
    cinf is the high frequency asymptote for capacitance\n\
    c0 is the low frequency asymptote for capacitance\n\
    tau0 is the average time constant of dielectric dipoles\n\
    alpha specifies the width of time constant distribution\n\
    esr is the equivalent series resistance\n\
    esl is the equivalent series inductance\n\
    rleak is the leakage resistance\n\
    lumps is number of lumps used the approximation\n\
	(use lumps < 0 to specify lumps/decade)\n\
    profile is determines whether the extreme critical frequenices\n\
	are poles or zeros\n\
	    `fd' implies flat at low freqs, down-slope at high freqs\n\
	    `dd' implies down-slope at low freqs, down-slope at high freqs\n\
	    `df' implies down-slope at low freqs, flat at high freqs\n\
	    `ff' implies flat at low freqs, flat at high freqs\n\
\n\
To print help message, use '-h', to print version, use '-V'\n";

    fprintf(stderr, intro, myname );
    fprintf(stderr, usage, myname );
}



