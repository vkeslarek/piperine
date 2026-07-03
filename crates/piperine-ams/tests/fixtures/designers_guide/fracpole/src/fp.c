/*
 * Generates an approximate model for a fractional impedance pole
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
double F1 = -1, F0 = -1, Coef = 1, Slope = -0.5;
char *myname = argv[0], *profile = "fd", *subcktName = "", fileName[BUFSIZ];
int i;
double size=0;
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
	else if (!strncmp(argv[i], "coef=", 5))
	    Coef = atof(argv[i]+5);
	else if (!strncmp(argv[i], "slope=", 6))
	    Slope = atof(argv[i]+6);
	else if (!strncmp(argv[i], "lumps=", 6))
	    size = atof(argv[i]+6);
	else if (!strncmp(argv[i], "profile=", 8))
	    profile = argv[i]+8;
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
    if (Coef <= 0 ) {
	fprintf(stderr, "Error: coef must be greater than 0.\n");
	fprintf(stderr, "    Use \"%s -h\" for more information\n", myname);
	exit(1);
    }
    if ((Slope <= -1) || (0 <= Slope)) {
	fprintf(stderr,
		"Error: slope must be greater than -1 and less that 0.\n");
	fprintf(stderr, "    Use \"%s -h\" for more information\n", myname);
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

/* Realize fractional impedance pole subcircuit */
    pFP = fpRealize( Coef, Slope, F0, F1, size, profileCode);
    if (pFP == NULL) {
	fprintf( stderr, "Error: insufficient memory available.");
	exit(1);
    }

/* Output realized circuit to a file in the form of a Spectre subcircuit */
    sprintf(fileName, "fracpole.scs");
    pF = fopen(fileName, "w");
    if (pF == NULL) {
	fprintf( stderr, "Error: cannot write to file `fracpole.scs'\n   %s.\n",
		 strerror( errno ));
	exit(1);
    }
    fprintf(pF, "simulator lang=spectre\n\n");

/* Print impedance fracpole subcircuit */
    fpPrint(pFP, pF, "fracpole", "coef", "");
    fpFree(pFP);

    fclose(pF);

/* Print expected asymptotic impedance. */
    pF = fopen("expected.plt", "w");
    fprintf(pF, "\"expected\n");
    fprintf(pF, "%lg\t%lg\n", F0, Coef*pow(2*PI*F0, Slope));
    fprintf(pF, "%lg\t%lg\n", F1, Coef*pow(2*PI*F1, Slope));
    fclose(pF);

    exit(0);
}



helpMessage(myname)
char *myname;
{
char *intro = "\
%s:\
    Generates a Spectre subcircuit description of fractional impedance\n\
    pole and places it in the file 'fracpole.scs'.  The impedance of a\n\
    fractional pole exhibits a slope equal to the negative of the number\n\
    of poles requested when plotted on a log-log scale. Thus, a half pole\n\
    exhibits a slope of -1/2. The user specifies the frequency range using\n\
    over which the approximation is valid using f0 and f1.\n\n";

char *usage = "\
Usage:\n\
%s f0=<val> f1=<val> [coef=<val>] [slope=<val>] [lumps=<val>] \\\n\
		   profile=<fd, dd, df, or ff>]\n\
where\n\
    f0 is the low frequency limit in hertz\n\
    f1 is the high frequency limit in hertz\n\
    coef is the unity intercept point for the ideal impedance (the magnitude\n\
	of the impedance when w=1 before approximation), default is 1.\n\
    slope is the slope of the impedance when plotted on a log-log scale\n\
	(equals the negative of the fraction of a pole desired). \n\
	Default is -0.5.\n\
    lumps is number of lumps used the approximation\n\
	(use lumps < 0 to specify lumps/decade)\n\
    profile is determines whether the extreme critical frequenices\n\
	of the impedance approximation are poles or zeros\n\
	    `fd' implies flat at low freqs, down-slope at high freqs\n\
	    `dd' implies down-slope at low freqs, down-slope at high freqs\n\
	    `df' implies down-slope at low freqs, flat at high freqs\n\
	    `ff' implies flat at low freqs, flat at high freqs.\n\
\n\
To print help message, use '-h', to print version, use '-V'\n";

    fprintf(stderr, intro, myname );
    fprintf(stderr, usage, myname );
}



